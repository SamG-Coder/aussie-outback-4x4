//! Watching another driver's run: a cinematic, which can be skipped (docs/campaign.md, The
//! others).
//!
//! It is not the whole lap, which is minutes of somebody else driving. It is the start, the
//! slowest places on the track, where time is won and lost, and the run to the line: a few
//! seconds each, seen from a camera standing on the track ahead of the truck, with the
//! driver's clock as it would read there. The truck is the map's own, in the other driver's
//! colour, driven by the game at their pace: the game draws one truck.
//!
//! This file plans the shots and follows them, and needs no game to try; the shell
//! (`shell.rs`) asks whoever runs the game for what each shot needs, a [`Stage`] at a time.

use crate::timed::{Course, Medal, Rival, CLASSES};
use glam::Vec3;
use serde_json::{json, Value};
use solstice_game::drive::Pace;
use solstice_game::par::RUN_UP;

/// What is asked of whoever runs the game, to stage a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    /// The game drives the truck at this pace, or gives it back to the player.
    Drive(Option<Pace>),
    /// The truck is painted this colour (as a page writes one, `0xRRGGBB`), or is given
    /// back the paint it had.
    Paint(Option<u32>),
    /// The truck is put on the track this far behind the start line, at rest, with no lap
    /// begun: below zero, that far round from it.
    Line(f32),
}

/// The paint the player's truck had before it was painted as somebody else's.
static OWN_PAINT: std::sync::Mutex<Option<Vec3>> = std::sync::Mutex::new(None);

/// Does what a [`Stage`] asks, in a game that is being played.
pub fn stage(play: &mut crate::play::PlayMode, stage: Stage) {
    match stage {
        Stage::Drive(pace) => play.play.set_autopilot(pace),
        Stage::Line(back) => {
            play.play.to_the_line(back);
        }
        Stage::Paint(colour) => {
            let mut wear = play.wear();
            let mut own = OWN_PAINT.lock().unwrap_or_else(|held| held.into_inner());
            match colour {
                Some(colour) => {
                    own.get_or_insert(wear.paint);
                    // As a page writes it, and so as the eye sees it: the renderer wants light.
                    let channel =
                        |shift: u32| (((colour >> shift) & 0xff) as f32 / 255.0).powf(2.2);
                    wear.paint = Vec3::new(channel(16), channel(8), channel(0));
                }
                None => match own.take() {
                    Some(paint) => wear.paint = paint,
                    None => return,
                },
            }
            play.set_wear(wear);
        }
    }
}

/// How long a shot lasts at least, and at most, in seconds.
const SHORTEST: f32 = 2.5;
const LONGEST: f32 = 12.0;
/// How near the camera the truck comes before the shot is cut, in metres.
const NEAR: f32 = 7.0;
/// How far apart the slow places shown are, at least, in metres.
const APART: f32 = 150.0;

/// One shot: where the truck is put, and where on the track the camera stands.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Shot {
    /// How far round the lap the truck starts, in metres: below zero, behind the line.
    from: f32,
    /// How far round the camera stands, which side of the middle of the track, and how high.
    eye: f32,
    side: f32,
    height: f32,
}

/// A run being watched.
#[derive(Clone, Debug, PartialEq)]
pub struct Cinema {
    pub rival: Rival,
    pub class: usize,
    /// The driver's time for the lap, in seconds.
    pub time: f32,
    shots: Vec<Shot>,
    shot: usize,
    /// How long this shot has run, where its camera is, and what it looks at.
    run: f32,
    eye: Vec3,
    look: Option<Vec3>,
    /// The driver's clock where the truck is, in seconds.
    clock: f32,
}

/// What a watched run needs next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Next {
    /// More of the same shot.
    Roll,
    /// Another shot: the truck is to be put here.
    Cut(Stage),
    /// It is over.
    Over,
}

impl Cinema {
    /// Plans the watching of one of a class's drivers round a course.
    pub fn plan(course: &Course, rival: Rival, class: usize, gold: f32) -> Self {
        let class = class.min(course.profiles.len() - 1);
        let profile = &course.profiles[class];
        let length = profile.length();
        let mut shots = vec![Shot {
            from: -RUN_UP,
            eye: 30.0_f32.min(length * 0.2),
            side: 1.0,
            height: 1.3,
        }];
        for (index, along) in profile
            .slowest(3, APART.min(length * 0.2))
            .iter()
            .enumerate()
        {
            shots.push(Shot {
                from: along - 18.0,
                eye: along + 14.0,
                side: if index % 2 == 0 { -1.0 } else { 1.0 },
                height: 1.5 + 0.4 * index as f32,
            });
        }
        shots.push(Shot {
            from: length - 45.0_f32.min(length * 0.2),
            // Well past the line, so that the truck is seen over it.
            eye: 16.0,
            side: -1.0,
            height: 1.5,
        });
        let mut cinema = Self {
            time: rival.medal.time(gold),
            rival,
            class,
            shots,
            shot: 0,
            run: 0.0,
            eye: Vec3::ZERO,
            look: None,
            clock: 0.0,
        };
        cinema.stand(course);
        cinema
    }

    /// The pace the truck is driven at: the class's, and slower for a slower medal.
    pub fn pace(&self) -> Pace {
        let (pace, slower) = (CLASSES[self.class].going.pace, self.rival.medal.slower());
        Pace {
            cornering: pace.cornering / (slower * slower),
            top_speed: pace.top_speed / slower,
            ..pace
        }
    }

    /// Where the truck is to be put for the shot there is now.
    pub fn puts(&self) -> Stage {
        Stage::Line(-self.shots[self.shot].from)
    }

    /// Stands the camera where this shot's is: on the track, to one side of its middle.
    fn stand(&mut self, course: &Course) {
        let shot = self.shots[self.shot];
        let (centre, forward, left) = course.track.place(shot.eye);
        let up = forward.cross(left).normalize_or(Vec3::Y);
        // Near the middle: a track is a truck wide, and what grows beside it is in the way.
        self.eye = centre + left * shot.side * 0.8 + up * shot.height;
        self.look = None;
        self.run = 0.0;
    }

    /// Time goes by.
    pub fn tick(&mut self, seconds: f32) {
        self.run += seconds;
    }

    /// Follows the truck: where it is, and how far round the lap. Says what is needed next.
    pub fn follow(&mut self, course: &Course, truck: Vec3, along: f32) -> Next {
        let profile = &course.profiles[self.class];
        let (first, last) = (self.shot == 0, self.shot + 1 == self.shots.len());
        // Behind the line at the start the clock has not begun; over it at the end, it has
        // stopped.
        let half = profile.length() * 0.5;
        self.clock = if first && along > half {
            0.0
        } else if last && along < half {
            self.time
        } else {
            (profile.time_at(along) * self.rival.medal.slower()).min(self.time)
        };
        self.look = Some(truck + Vec3::Y * 1.0);
        let near = (truck.x - self.eye.x).hypot(truck.z - self.eye.z) < NEAR;
        if self.run < SHORTEST || (!near && self.run < LONGEST) {
            return Next::Roll;
        }
        if last {
            return Next::Over;
        }
        self.shot += 1;
        self.stand(course);
        Next::Cut(self.puts())
    }

    /// Where the camera is and the way it looks, as a yaw and a pitch in radians, once
    /// there is a truck to look at.
    pub fn view(&self) -> Option<(Vec3, f32, f32)> {
        let to = self.look? - self.eye;
        Some((self.eye, (-to.x).atan2(-to.z), to.y.atan2(to.x.hypot(to.z))))
    }

    /// As the pages are told of it.
    pub fn told(&self) -> Value {
        json!({
            "name": self.rival.name,
            "truck": self.rival.truck,
            "paint": format!("#{:06x}", self.rival.paint),
            "medal": self.rival.medal.name(),
            "class": self.class,
            "time": self.time,
            "clock": self.clock,
            "shot": self.shot + 1,
            "shots": self.shots.len(),
        })
    }
}

/// Which of a class's drivers a page's word for one is: `watch0` is bronze's.
pub fn chosen(what: &str) -> Option<Medal> {
    let index: usize = what.strip_prefix("watch")?.parse().ok()?;
    Medal::ALL.get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timed::rivals;
    use solstice_asset::GameFile;

    /// A ring of track 200 m across, flat, with a bog on its far side.
    fn ring() -> Course {
        let mut track = String::from("name Ring\n");
        for step in 0..157 {
            let angle = step as f32 / 157.0 * std::f32::consts::TAU;
            // Round from the west, heading north first: the left is out from the middle.
            let (x, z) = (-100.0 * angle.cos(), -100.0 * angle.sin());
            track += &format!(
                "p {x:.3} 5 {z:.3} {:.4} 0 {:.4} 2 2\n",
                -angle.cos(),
                -angle.sin()
            );
        }
        // Squares of ground ten metres wide: bog where the track crosses the east side.
        let mut ground = solstice_game::SurfaceMap {
            origin: [-120.0, -120.0],
            spacing: 10.0,
            columns: 24,
            rows: 24,
            squares: vec![0; 24 * 24],
        };
        // Which surface is which is the engine's to say: found by asking.
        let named = |name: &str| {
            (0..32_u8)
                .find(|&index| {
                    let mut one = ground.clone();
                    one.squares.fill(index);
                    one.at(Vec3::ZERO).name() == name
                })
                .expect("there is such a surface")
        };
        let (dirt, bog) = (named("dirt"), named("bog"));
        for row in 0..24 {
            for column in 0..24 {
                let wet = column >= 21 && (11..=12).contains(&row);
                ground.squares[row * 24 + column] = if wet { bog } else { dirt };
            }
        }
        let ground = ground.to_text();
        let files = [
            GameFile {
                name: "outback.track".to_owned(),
                text: track,
            },
            GameFile {
                name: "outback.surfaces".to_owned(),
                text: ground,
            },
        ];
        Course::read(&files).expect("a ring is a course")
    }

    #[test]
    fn a_run_is_watched_from_the_start_through_the_slow_places_to_the_line() {
        let course = ring();
        let gold = course.golds()[2];
        let rival = rivals("ring", 2)[1].clone();
        let mut cinema = Cinema::plan(&course, rival, 2, gold);
        assert_eq!(cinema.time, Medal::Silver.time(gold));
        // It starts behind the line, with nothing to look at until the truck is seen.
        assert_eq!(cinema.puts(), Stage::Line(RUN_UP));
        assert_eq!(cinema.view(), None);
        // A slower medal's driver is driven slower.
        assert!(cinema.pace().top_speed < CLASSES[2].going.pace.top_speed);
        let length = course.track.length();
        let mut shots = 1;
        let mut along = length - RUN_UP;
        let mut clock = -1.0;
        for _ in 0..4000 {
            cinema.tick(0.05);
            along += 0.4;
            let (truck, _, _) = course.track.place(along);
            match cinema.follow(&course, truck, along.rem_euclid(length)) {
                Next::Roll => {
                    // The camera stands on the track and looks at the truck.
                    let (eye, _, pitch) = cinema.view().expect("there is a truck to see");
                    assert!(course.track.locate(eye, None).offset.abs() < 2.0);
                    assert!(pitch.abs() < 1.2);
                }
                Next::Cut(Stage::Line(back)) => {
                    // The truck is put where the next shot begins.
                    shots += 1;
                    along = -back;
                }
                Next::Cut(other) => panic!("a cut puts the truck somewhere: {other:?}"),
                Next::Over => break,
            }
            // The driver's clock only goes on, and stops at their time.
            let told = cinema.told();
            let now = told["clock"].as_f64().expect("a clock");
            assert!(now >= clock - 1.0e-3 && now <= f64::from(cinema.time) + 1.0e-3);
            clock = now;
        }
        assert!(
            shots >= 3,
            "the start, a slow place and the finish: {shots}"
        );
        assert_eq!(cinema.told()["shot"], cinema.told()["shots"]);
        assert!((clock - f64::from(cinema.time)).abs() < 1.0e-3, "{clock}");
    }

    #[test]
    fn a_shot_is_cut_when_the_truck_is_stuck() {
        let course = ring();
        let rival = rivals("ring", 0)[2].clone();
        let mut cinema = Cinema::plan(&course, rival, 0, course.golds()[0]);
        let (truck, _, _) = course.track.place(-RUN_UP);
        let mut cuts = 0;
        for _ in 0..10_000 {
            cinema.tick(0.05);
            match cinema.follow(&course, truck, course.track.length() - RUN_UP) {
                Next::Roll => {}
                Next::Cut(_) => cuts += 1,
                Next::Over => break,
            }
        }
        // Going nowhere, every shot runs its longest and the watching still ends.
        assert!(cuts >= 2, "{cuts}");
        assert_eq!(cinema.told()["shot"], cinema.told()["shots"]);
    }

    #[test]
    fn a_page_names_the_driver_to_watch() {
        assert_eq!(chosen("watch0"), Some(Medal::Bronze));
        assert_eq!(chosen("watch2"), Some(Medal::Gold));
        assert_eq!(chosen("watch3"), None);
        assert_eq!(chosen("watch"), None);
    }
}
