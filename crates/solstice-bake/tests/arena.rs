//! Bulldust Bowl, driven: a truck goes round the loop and comes off it on its wheels, and
//! goes over a jump with all four of them off the ground.
//!
//! The land is the design's own grid of heights, and the loop is its deck and its rails as
//! the triangles they are drawn with: what the bake puts in the map, less the few
//! centimetres its collision is allowed to be out by. Nothing is special to the loop: the
//! truck's wheels find the deck along the truck's own down, whichever way up it is.

use glam::{Mat4, Vec3};
use solstice_asset::{Collision, CollisionShape, GameFile, Heightfield, NO_SHAPE};
use solstice_bake::outback::arena::ring::WIDE;
use solstice_bake::outback::arena::{Design, AROUND, JUMPS, LEVEL};
use solstice_builder::Mesh;
use solstice_game::{Devices, Play, SurfaceMap, GAME_FILE};
use solstice_physics::vehicle::VehicleSpec;
use solstice_physics::Car;
use solstice_world::STEP_SECONDS;

const KEYS: &str = "throttle key KeyW\nbrake key KeyS\nsteer key KeyD\nsteer key KeyA -1\n";

/// The trucks it is tried with: a full-size desert truck, and an old wagon half its power.
const KESTREL: &str = include_str!("../../../scenes/trucks/kestrel.vehicle");
const HOWITT: &str = include_str!("../../../scenes/trucks/howitt.vehicle");
/// And what a monster truck stands on: 3.9 m wide on tyres 1.7 m high.
const MONSTER: &str = include_str!("../../../scenes/trucks/monster.vehicle");

fn shape(mesh: &Mesh) -> CollisionShape {
    CollisionShape {
        hull: false,
        vertices: mesh
            .vertices
            .iter()
            .map(|v| v.position.to_array())
            .collect(),
        triangles: mesh.indices.chunks(3).map(|t| [t[0], t[1], t[2]]).collect(),
    }
}

/// The arena as it is driven on, with this truck put at a place, facing a way.
fn arena(design: &Design, truck: &str, at: Vec3, yaw: f32) -> Play {
    let spacing = 0.5;
    let cells = (2.0 * AROUND / spacing).round() as u32;
    let ground = Heightfield {
        columns: cells + 1,
        rows: cells + 1,
        origin: [-AROUND, -AROUND],
        spacing: [spacing, spacing],
        heights: (0..(cells + 1) * (cells + 1))
            .map(|index| {
                design.height(
                    (index % (cells + 1)) as f32 * spacing - AROUND,
                    (index / (cells + 1)) as f32 * spacing - AROUND,
                )
            })
            .collect(),
    };
    // What a tyre is on, a metre to the square: enough to know the deck from the dirt.
    let squares = (2.0 * AROUND) as u32;
    let surfaces = SurfaceMap {
        origin: [-AROUND, -AROUND],
        spacing: 1.0,
        columns: squares,
        rows: squares,
        squares: (0..squares * squares)
            .map(|index| {
                design
                    .surface(
                        (index % squares) as f32 + 0.5 - AROUND,
                        (index / squares) as f32 + 0.5 - AROUND,
                    )
                    .index()
            })
            .collect(),
    };
    let meshes = design.ring.meshes();
    let collision = Collision {
        shapes: vec![shape(&meshes.deck), shape(&meshes.rails)],
        // The truck's body and wheels, which collide with nothing of the scene's; then the
        // deck and its rails.
        instance_shapes: vec![
            NO_SHAPE, NO_SHAPE, NO_SHAPE, NO_SHAPE, NO_SHAPE, NO_SHAPE, 0, 1,
        ],
        heightfield: Some(ground),
    };
    let spec = VehicleSpec::parse(truck).expect("the truck's file reads");
    let hubs = spec.hubs();
    let scene = format!(
        "input keys.input\nsurfaces arena.surfaces\nactor truck.actor {} {} {} {yaw} instances=1+5\n",
        at.x, at.y, at.z
    );
    let files: Vec<GameFile> = [
        (GAME_FILE, scene.as_str()),
        (
            "truck.actor",
            "name truck\nmesh truck.glb 1\nvehicle truck.vehicle\ncamera 0 1.6 0\n",
        ),
        ("truck.vehicle", truck),
        ("keys.input", KEYS),
        ("arena.surfaces", &surfaces.to_text()),
    ]
    .iter()
    .map(|(name, text)| GameFile {
        name: (*name).to_owned(),
        text: (*text).to_owned(),
    })
    .collect();
    let placed = design.ring.placed;
    let start = Mat4::from_translation(at) * Mat4::from_rotation_y(yaw.to_radians());
    let baked = move |id: u32| match id {
        0 | 1 => Some(start),
        2..=5 => Some(start * Mat4::from_translation(hubs[id as usize - 2])),
        6 | 7 => Some(placed),
        _ => None,
    };
    Play::load(&files, baked, Some(&collision)).unwrap_or_else(|e| panic!("{e:?}"))
}

/// Where the truck is, which way its roof points, how fast it is going along itself, and
/// how many of its wheels are on something.
fn truck(play: &Play) -> (Vec3, Vec3, f32, usize) {
    let (at, turned) = play.car_pose().expect("a truck");
    let world = &play.world().entities;
    let mut query = world.query::<&Car>();
    let car = query.iter().next().expect("a car");
    let wheels = car.vehicle.wheels.iter().filter(|w| w.on_ground).count();
    (at, turned * Vec3::Y, car.vehicle.telemetry().speed, wheels)
}

fn step(play: &mut Play, devices: &mut Devices) {
    play.frame(STEP_SECONDS, devices, |_, _| {});
}

/// What happened on a run at the loop.
#[derive(Debug)]
struct Round {
    /// How fast it came on to the deck, the highest it got and how fast it was going
    /// there, and the hardest it was pressed into the deck, in g.
    entry: f32,
    top: f32,
    over: f32,
    pressed: f32,
    /// Whether it was ever upside down with its wheels on the deck; and how far it was
    /// from the middle of the deck at the most, while it was on it.
    inverted: bool,
    wandered: f32,
    /// Where it ended, which way up, how fast, and on how many wheels: twenty metres past
    /// the loop, if it got that far, or when the time was up.
    end: Vec3,
    up: f32,
    speed: f32,
    wheels: usize,
    /// Whether it came off the far end of the deck.
    came_off: bool,
}

/// Drives a truck at the loop from `back` metres before it, foot down and wheel straight,
/// until it is twenty metres past it or `seconds` are up.
fn round(spec: &str, back: f32, seconds: f32) -> Round {
    let design = Design::new();
    let ring = &design.ring;
    let (from, yaw) = ring.approach(back);
    let from = Vec3::new(from.x, design.height(from.x, from.z) + 0.05, from.z);
    let mut play = arena(&design, spec, from, yaw);
    let mut devices = Devices::new();
    for _ in 0..60 {
        step(&mut play, &mut devices);
    }
    devices.key("KeyW", true);
    let (on, _) = ring.on_map(0.0, 0.0);
    let ahead = (ring.way_off(10.0) - ring.way_off(0.0)).normalize();
    let mut round = Round {
        entry: 0.0,
        top: 0.0,
        over: 0.0,
        pressed: 0.0,
        inverted: false,
        wandered: 0.0,
        end: Vec3::ZERO,
        up: 0.0,
        speed: 0.0,
        wheels: 0,
        came_off: false,
    };
    for _ in 0..(seconds / STEP_SECONDS as f32) as usize {
        step(&mut play, &mut devices);
        let (at, up, speed, wheels) = truck(&play);
        if round.entry == 0.0 && at.distance(on) < 1.5 {
            round.entry = speed;
        }
        if at.y > round.top {
            (round.top, round.over) = (at.y, speed);
        }
        if up.y < -0.9 && wheels >= 3 {
            round.inverted = true;
        }
        if round.entry > 0.0 {
            let pressed = {
                let world = &play.world().entities;
                let mut query = world.query::<&Car>();
                let car = query.iter().next().expect("a car");
                car.vehicle.telemetry().g.z
            };
            round.pressed = round.pressed.max(pressed);
            let (_, across, off) = on_loop(&design, at);
            if off < 1.0 && at.y > LEVEL + 3.0 {
                round.wandered = round.wandered.max(across.abs());
            }
        }
        (round.end, round.up, round.speed, round.wheels) = (at, up.y, speed, wheels);
        // Past the loop, on the ground, having been over the top of it.
        if round.inverted && (at - ring.way_off(20.0)).dot(ahead) > 0.0 && at.y < LEVEL + 1.0 {
            round.came_off = true;
            break;
        }
    }
    round
}

#[test]
fn a_truck_goes_round_the_loop_and_comes_off_it_on_its_wheels() {
    let design = Design::new();
    let ring = &design.ring;
    let off = ring.way_off(20.0);
    // A full-size truck from a short run and from a long one, and an old wagon with half
    // its power from the whole of the run-up there is.
    for (name, spec, back) in [
        ("kestrel", KESTREL, 45.0),
        ("kestrel", KESTREL, 100.0),
        ("howitt", HOWITT, 100.0),
        ("monster", MONSTER, 60.0),
        ("monster", MONSTER, 100.0),
    ] {
        let round = round(spec, back, 20.0);
        println!("{name} from {back} m back: {round:?}");
        // It was upside down at the top, on its wheels, and going fast enough to stay on.
        assert!(round.inverted, "{name}: {round:?}");
        assert!(round.top > LEVEL + ring.height() - 2.2, "{name}: {round:?}");
        assert!(round.over > 7.0, "{name}: {round:?}");
        // It kept to the deck, with its wheel held straight all the way round.
        assert!(round.wandered < WIDE * 0.5, "{name}: {round:?}");
        // And it came off the far end on its wheels, still going, on the way out: beside
        // the way in, further on.
        assert!(round.came_off, "{name}: {round:?}");
        assert!(
            round.up > 0.97 && round.speed > 8.0 && round.wheels == 4,
            "{name}: {round:?}"
        );
        assert!(
            round.end.distance(off) < 3.5,
            "{name}: {round:?}, the way out is {off}"
        );
    }
}

#[test]
fn a_truck_that_is_too_slow_for_the_loop_does_not_get_round_it() {
    // From a few metres back it comes on at a walk, even with its foot down: it climbs
    // part of the way, and never gets to the top.
    let round = round(HOWITT, 6.0, 9.0);
    println!("howitt from 6 m back: {round:?}");
    assert!(!round.inverted && !round.came_off, "{round:?}");
    assert!(
        round.top < LEVEL + Design::new().ring.height() - 4.0,
        "{round:?}"
    );
}

#[test]
fn a_truck_flies_off_a_jump_and_lands_on_its_wheels() {
    let design = Design::new();
    // Each jump, and the run it is taken from: far enough back to clear it, and no more.
    for (name, back) in [
        ("hop", 22.0),
        ("tabletop", 26.0),
        ("double", 34.0),
        ("step-up", 30.0),
        ("big air", 42.0),
    ] {
        let jump = JUMPS
            .iter()
            .find(|jump| jump.name == name)
            .expect("it is one of them");
        let from = jump.along(-back);
        let yaw = (-jump.heading.x).atan2(-jump.heading.y).to_degrees();
        let from = Vec3::new(from.x, design.height(from.x, from.y) + 0.05, from.y);
        let mut play = arena(&design, KESTREL, from, yaw);
        let mut devices = Devices::new();
        for _ in 0..60 {
            step(&mut play, &mut devices);
        }
        devices.key("KeyW", true);
        let (mut airborne, mut highest, mut flown, mut lip) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0);
        let mut left = None;
        let mut landed = None;
        let mut touched = None;
        for _ in 0..(9.0 / STEP_SECONDS) as usize {
            step(&mut play, &mut devices);
            let (at, up, speed, wheels) = truck(&play);
            let along = (glam::Vec2::new(at.x, at.z) - jump.at).dot(jump.heading);
            if wheels == 0 && along > 0.0 && landed.is_none() {
                airborne += STEP_SECONDS as f32;
                highest = highest.max(at.y - design.height(at.x, at.z));
                if left.is_none() {
                    lip = speed;
                }
                let from = *left.get_or_insert(along);
                flown = flown.max(along - from);
            }
            // Off the throttle once it is in the air.
            if left.is_some() {
                devices.key("KeyW", false);
            }
            // Down again, on three wheels or four: where it is half a second after.
            if left.is_some() && wheels >= 3 && touched.is_none() {
                touched = Some(0.0_f32);
            }
            if let Some(since) = touched.as_mut() {
                *since += STEP_SECONDS as f32;
                if *since >= 0.5 && landed.is_none() {
                    landed = Some((at, up, speed));
                }
            }
        }
        let last = truck(&play);
        println!("{name}: at the end {last:?}");
        println!(
            "{name}: off the lip at {lip:.1} m/s, in the air {airborne:.2} s, {highest:.2} m off the ground, {flown:.1} m; down at {landed:?}"
        );
        assert!(airborne > 0.35 && highest > 0.5 && flown > 5.0, "{name}");
        // It came down past the lip it left by, the right way up, in its lane and still
        // rolling the way it went.
        let (at, up, speed) = landed.unwrap_or_else(|| panic!("{name}: it never came down"));
        let off = glam::Vec2::new(at.x, at.z) - jump.at;
        let (along, aside) = (off.dot(jump.heading), off.dot(jump.heading.perp()));
        assert!(
            up.y > 0.9 && speed > 3.0 && aside.abs() < jump.half && along > 12.0,
            "{name}: {at} {up} {speed}"
        );
    }
}

/// Where a place is on the loop: how far along its deck, how far to the right of its
/// middle, and how far off it.
fn on_loop(design: &Design, at: Vec3) -> (f32, f32, f32) {
    let ring = &design.ring;
    let mut best = (0.0, f32::MAX);
    let mut run = -5.0;
    while run < ring.length() + 5.0 {
        let (middle, _) = ring.on_map(run, 0.0);
        let (right, _) = ring.on_map(run, 1.0);
        let aside = right - middle;
        let off = at - middle;
        let level = off - aside * off.dot(aside);
        if level.length() < best.1 {
            best = (run, level.length());
        }
        run += 0.1;
    }
    let (middle, _) = ring.on_map(best.0, 0.0);
    let (right, _) = ring.on_map(best.0, 1.0);
    (best.0, (at - middle).dot(right - middle), best.1)
}

#[test]
#[ignore]
fn trace_a_run_at_the_loop() {
    let design = Design::new();
    let back: f32 = std::env::var("LOOP_BACK")
        .ok()
        .and_then(|b| b.parse().ok())
        .unwrap_or(60.0);
    let (from, yaw) = design.ring.approach(back);
    let from = Vec3::new(from.x, design.height(from.x, from.z) + 0.05, from.z);
    let mut play = arena(&design, KESTREL, from, yaw);
    let mut devices = Devices::new();
    for _ in 0..60 {
        step(&mut play, &mut devices);
    }
    devices.key("KeyW", true);
    for tick in 0..(14.0 / STEP_SECONDS) as usize {
        step(&mut play, &mut devices);
        let close = std::env::var("LOOP_CLOSE").is_ok() && (440..475).contains(&tick);
        if close {
            let touches: Vec<Vec3> = {
                let world = &play.world().entities;
                let mut query = world.query::<(solstice_world::Entity, &Car)>();
                let (entity, _) = query.iter().next().expect("a car");
                play.physics().car_touches(entity)
            };
            println!("   touches {touches:?}");
        }
        if tick % 12 == 0 || close {
            let (at, up, speed, wheels) = truck(&play);
            let (run, across, off) = on_loop(&design, at);
            let world = &play.world().entities;
            let mut query = world.query::<&Car>();
            let car = query.iter().next().expect("a car");
            let g = car.vehicle.telemetry().g;
            println!(
                "{:5.2}s at {:6.1} {:5.1} {:6.1}  up {:5.2} {:5.2} {:5.2}  {:5.1} m/s  wheels {wheels}  run {run:5.1} across {across:5.2} off {off:4.2}  g {:4.1} {:4.1} {:4.1}",
                tick as f64 * STEP_SECONDS, at.x, at.y, at.z, up.x, up.y, up.z, speed, g.x, g.y, g.z
            );
        }
    }
}
