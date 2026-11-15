//! The town: a hotel, a general store, the gold buyer's and a hall along one side of a yard
//! off the main road, where what has been found is sold.
//!
//! The buildings are a model, made in Blender by `tools/blender/town/build.py` and kept out of
//! the repository as what that script makes (`target/outback/town.glb`; `bake.bat` runs it).
//! This is where it stands, and what the game has to know of it that a picture does not say:
//! which parts of it stop a truck, where the gold buyer's is, and where its lamps hang. The
//! measurements are the script's own, in the model's own plan: x to the right of somebody
//! facing the buildings' fronts, y away from them toward the buildings, in metres from the
//! middle of the yard.
use glam::{Mat4, Quat, Vec2, Vec3};
use solstice_asset::{Light, SceneAsset};
use solstice_game::Blocker;
use solstice_import::{append_model, import_gltf, ImportOptions, ModelPlacements};
use std::path::Path;

/// The model, from the folder a scene's text is in.
const MODEL: &str = "../target/outback/town.glb";
/// How far the middle of its yard is from the middle of the road.
const AWAY: f32 = 15.5;
/// Its ground, the yard and what the buildings stand on: from, and to, across and deep.
const GROUND: ([f32; 2], [f32; 2]) = ([-31.0, -11.0], [30.0, 19.0]);
/// How far round its ground the land eases back to what it was.
const EASE: f32 = 9.0;
/// The buildings' front walls, how deep their verandahs are, and each building: from, and
/// to, across the front, and how deep it is behind its front wall.
const FRONT: f32 = 6.0;
const DECK: f32 = 2.6;
/// How high the floors are above the ground: a verandah's deck is stepped up onto.
const FLOOR: f32 = 0.32;
/// How many posts each verandah stands on, along its front edge: the hall has a porch.
const POSTS: [usize; 4] = [7, 5, 4, 0];
const BUILDINGS: [[f32; 3]; 4] = [
    // The hotel, the general store, the gold buyer's, the hall.
    [-27.0, -13.0, 9.0],
    [-10.0, 0.0, 9.0],
    [3.0, 11.0, 7.0],
    [14.0, 26.0, 11.0],
];
/// Which of them is the gold buyer's: a truck stops in the yard before it to sell.
const BUYER: usize = 2;
/// How far in front of the buyer's verandah the middle of that place is, and how far round.
const BAY: [f32; 2] = [3.4, 5.5];
/// How high a lamp hangs under a verandah.
const LAMP: f32 = 2.5;

/// Where the town stands.
#[derive(Clone, Copy, Debug)]
pub struct Town {
    /// The middle of its yard, and the ground's height there.
    pub at: Vec2,
    pub height: f32,
    /// The way its buildings face: toward the road.
    front: Vec2,
}

impl Town {
    /// Beside the road where a truck starts on it, which runs `forward` there: on the left
    /// of somebody driving off, facing the road.
    pub fn beside(start: Vec2, forward: Vec2, height: f32) -> Self {
        let left = Vec2::new(forward.y, -forward.x).normalize();
        Self {
            at: start + left * AWAY,
            height,
            front: -left,
        }
    }

    /// A place in the model's plan, on the map.
    fn place(&self, x: f32, y: f32) -> Vec2 {
        self.at + Vec2::new(self.front.y, -self.front.x) * x - self.front * y
    }

    /// A place on the map, in the model's plan.
    fn plan(&self, x: f32, z: f32) -> Vec2 {
        let off = Vec2::new(x, z) - self.at;
        Vec2::new(
            off.dot(Vec2::new(self.front.y, -self.front.x)),
            -off.dot(self.front),
        )
    }

    /// How far a place is outside its ground: nothing, on it.
    fn outside(&self, x: f32, z: f32) -> f32 {
        let at = self.plan(x, z);
        let (low, high) = (Vec2::from(GROUND.0), Vec2::from(GROUND.1));
        (low - at).max(at - high).max(Vec2::ZERO).length()
    }

    /// Whether a place is on its ground, or within `margin` of it: nothing grows there.
    pub fn covers(&self, x: f32, z: f32, margin: f32) -> bool {
        self.outside(x, z) <= margin
    }

    /// How much the ground at a place is held to the yard's level: all of it on the town's
    /// ground, easing off to none over a good way, as a town's edges do.
    pub fn held(&self, x: f32, z: f32) -> f32 {
        let away = self.outside(x, z);
        let eased = 1.0 - (away / EASE).clamp(0.0, 1.0);
        eased * eased * (3.0 - 2.0 * eased)
    }

    /// The turn that faces it at the road, about the upright, in radians.
    fn yaw(&self) -> f32 {
        self.front.x.atan2(self.front.y)
    }

    /// Where a truck stops to sell gold, in the yard before the buyer's: where, and how
    /// far round.
    pub fn buyer(&self) -> (Vec3, f32) {
        let [from, to, _] = BUILDINGS[BUYER];
        let at = self.place((from + to) * 0.5, FRONT - DECK - BAY[0]);
        (Vec3::new(at.x, self.height, at.y), BAY[1])
    }

    /// The doors that are walked up to, each with what is behind it: the used trucks at the
    /// hotel, the auto shop at the store, and the bank in the hall. Where somebody stands
    /// before each, on the deck.
    pub fn doors(&self) -> Vec<(&'static str, Vec3)> {
        let middle = |index: usize| (BUILDINGS[index][0] + BUILDINGS[index][1]) * 0.5;
        [
            ("used", BUILDINGS[0][0] + 3.6),
            ("shop", middle(1)),
            ("bank", middle(3)),
        ]
        .into_iter()
        .map(|(what, x)| {
            let at = self.place(x, FRONT - 0.8);
            (what, Vec3::new(at.x, self.height + FLOOR, at.y))
        })
        .collect()
    }

    /// Each building as a truck meets it and as somebody on foot does: its walls, which
    /// stop both; its verandah's deck, which is low, and is stepped up onto to come to its
    /// doors; and the posts along the deck's edge.
    pub fn blockers(&self) -> Vec<Blocker> {
        let mut blockers = Vec::new();
        let mut boxed = |from: [f32; 2], to: [f32; 2], high: f32| {
            let at = self.place((from[0] + to[0]) * 0.5, (from[1] + to[1]) * 0.5);
            blockers.push(Blocker {
                centre: Vec3::new(at.x, self.height + high * 0.5, at.y),
                size: Vec3::new(to[0] - from[0], high, to[1] - from[1]),
                yaw_degrees: self.yaw().to_degrees(),
            });
        };
        for ([from, to, deep], posts) in BUILDINGS.into_iter().zip(POSTS) {
            boxed(
                [from - 0.3, FRONT - 0.06],
                [to + 0.3, FRONT + deep + 0.3],
                7.0,
            );
            if posts == 0 {
                continue;
            }
            boxed(
                [from - 0.03, FRONT - DECK],
                [to + 0.03, FRONT - 0.06],
                FLOOR,
            );
            for post in 0..posts {
                let x = from + 0.12 + (to - from - 0.24) * post as f32 / (posts - 1) as f32;
                let y = FRONT - DECK + 0.1;
                boxed([x - 0.09, y - 0.09], [x + 0.09, y + 0.09], 2.75);
            }
        }
        // The hotel's chimney, outside its end wall; and the hall's porch: a landing before
        // its doors, and the two posts its little gable stands on.
        let [hotel, _, deep] = BUILDINGS[0];
        boxed(
            [hotel - 0.9, FRONT + deep * 0.55 - 0.7],
            [hotel, FRONT + deep * 0.55 + 0.7],
            7.0,
        );
        let [from, to, _] = BUILDINGS[3];
        let middle = (from + to) * 0.5;
        boxed(
            [middle - 1.7, FRONT - 1.9],
            [middle + 1.7, FRONT - 0.06],
            FLOOR,
        );
        for side in [-1.6, 1.6] {
            let (x, y) = (middle + side, FRONT - 1.8);
            boxed([x - 0.09, y - 0.09], [x + 0.09, y + 0.09], 2.75);
        }
        blockers
    }

    /// Puts the model in the scene, and a lamp under each verandah. `base` is the folder the
    /// scene's text is in.
    pub fn build(&self, scene: &mut SceneAsset, base: &Path) -> Result<(), String> {
        let path = base.join(MODEL);
        if !path.exists() {
            return Err(format!(
                "{} is missing: the town is made by `blender -b --python tools/blender/town/build.py -- target/outback/town.glb` (bake.bat does it)",
                path.display()
            ));
        }
        let options = ImportOptions {
            texture_size: scene.texture_size,
            ..Default::default()
        };
        let model = import_gltf(&path, &options).map_err(|e| format!("{}: {e}", path.display()))?;
        let transform = Mat4::from_rotation_translation(
            Quat::from_rotation_y(self.yaw()),
            Vec3::new(self.at.x, self.height, self.at.y),
        );
        // What stops a truck is said in boxes: the model itself is only looked at.
        append_model(
            scene,
            model,
            &ModelPlacements {
                whole: vec![transform],
                parts: Vec::new(),
            },
        )?;
        for [from, to, _] in BUILDINGS {
            let at = self.place((from + to) * 0.5, FRONT - DECK * 0.5);
            scene.lights.push(Light {
                position: [at.x, self.height + LAMP, at.y],
                range: 11.0,
                intensity: [2.2, 1.3, 0.55],
                _pad: 0,
            });
        }
        let (buyer, _) = self.buyer();
        println!(
            "  the town stands at {:.2} {:.2} {:.2}, its gold buyer's at {:.2} {:.2}",
            self.at.x, self.height, self.at.y, buyer.x, buyer.z
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_town_stands_back_from_the_road_facing_it_with_room_to_park_at_the_buyers() {
        // A road running north at the origin: the town is to its west, facing east.
        let town = Town::beside(Vec2::ZERO, Vec2::new(0.0, -1.0), 40.0);
        assert!((town.at - Vec2::new(-AWAY, 0.0)).length() < 1e-4);
        // The front of its ground comes to the road's edge, and no building is that near.
        let edge = town.place(0.0, GROUND.0[1]);
        assert!(edge.x < -3.4 && edge.x > -6.0, "{edge}");
        assert!(town.covers(edge.x, edge.y, 0.01) && !town.covers(0.0, 0.0, 0.5));
        assert_eq!(town.held(town.at.x, town.at.y), 1.0);
        assert!(town.held(-AWAY - 60.0, 0.0) == 0.0);
        // A place is where its plan says, and back.
        let there = town.place(7.0, 12.0);
        assert!((town.plan(there.x, there.y) - Vec2::new(7.0, 12.0)).length() < 1e-4);
        // Every building is on the town's ground and stops a truck, and the buyer's place is
        // in the open yard before its verandah: in none of them.
        let blockers = town.blockers();
        // (Four buildings, three decks and their sixteen posts, a chimney, and a porch on two.)
        assert_eq!(blockers.len(), 4 + 3 + 16 + 1 + 3);
        // A verandah is walked onto: nothing taller than a step is between the yard and a
        // building's front wall but its posts.
        for [from, to, _] in &BUILDINGS[..3] {
            let door = town.place((from + to) * 0.5 + 0.6, FRONT - 0.5);
            let tall = blockers
                .iter()
                .filter(|blocker| blocker.size.y > 0.6)
                .any(|blocker| {
                    let off =
                        town.plan(blocker.centre.x, blocker.centre.z) - town.plan(door.x, door.y);
                    off.x.abs() < blocker.size.x * 0.5 && off.y.abs() < blocker.size.z * 0.5
                });
            assert!(!tall, "the way to a door is walled off");
        }
        let (bay, round) = town.buyer();
        let bay_plan = town.plan(bay.x, bay.z);
        assert!(bay_plan.x > 3.0 && bay_plan.x < 11.0 && bay_plan.y < FRONT - DECK - round * 0.5);
        for blocker in &blockers {
            assert!(town.covers(blocker.centre.x, blocker.centre.z, 0.0));
            let off = town.plan(blocker.centre.x, blocker.centre.z) - bay_plan;
            let clear = off.x.abs() > blocker.size.x * 0.5 + 1.2
                || off.y.abs() > blocker.size.z * 0.5 + 1.2;
            assert!(
                clear,
                "a truck in the buyer's place is in a building: {blocker:?}"
            );
        }
    }
}
