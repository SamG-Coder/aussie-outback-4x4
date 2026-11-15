//! The shed at the Crossroads' camp: where the truck is parked and its driver sleeps.
//!
//! The shed itself is a model, made in Blender by `tools/blender/shed/build.py` and kept out
//! of the repository as what that script makes (`target/outback/shed.glb`; `bake.bat` runs
//! it). This is where it stands, and what the game has to know of it that a picture does not
//! say: which parts of it stop a truck, where the bay is that a truck is parked in, and where
//! its lanterns hang. The measurements are the script's own, in the model's own plan: x to
//! the right of somebody facing its open front, y away from them into it, in metres from the
//! middle of its slab.
use glam::{Mat4, Quat, Vec2, Vec3};
use solstice_asset::{Light, SceneAsset};
use solstice_game::Blocker;
use solstice_import::{append_model, import_gltf, ImportOptions, ModelPlacements};
use std::path::Path;

/// The model, from the folder a scene's text is in.
const MODEL: &str = "../target/outback/shed.glb";
/// How far the middle of its slab is from the middle of the road.
const AWAY: f32 = 12.5;
/// Its slab, with the tank beyond its right-hand end: from, and to, across and deep.
const SLAB: ([f32; 2], [f32; 2]) = ([-5.3, -3.85], [7.6, 3.85]);
/// What stops a truck: each a box, its middle and its size across and deep.
const SOLID: [[f32; 4]; 10] = [
    // The walls: the back, the two ends, the room's wall on the bay, and its front on
    // either side of its doorway.
    [0.0, 3.57, 10.4, 0.22],
    [-5.07, 0.0, 0.22, 7.2],
    [5.07, 1.55, 0.22, 3.95],
    [-0.07, 1.55, 0.22, 3.95],
    [0.37, -0.4, 0.74, 0.22],
    [3.33, -0.4, 3.34, 0.22],
    // The front's three posts.
    [-5.0, -3.5, 0.3, 0.3],
    [0.0, -3.5, 0.3, 0.3],
    [5.0, -3.5, 0.3, 0.3],
    // The tank on its stand.
    [6.35, 2.4, 2.2, 2.2],
];
/// In the bay, the chest: its middle, its size across and deep, how high its floor is inside
/// and how high its rim. It stands open, and is hollow: what is put in it lies in it.
const CHEST: ([f32; 2], [f32; 2], f32, f32) = ([-0.62, 1.55], [0.56, 1.15], 0.128, 0.548);
/// How thick its sides are, as things meet them.
const CHEST_SIDE: f32 = 0.05;
/// The bay a truck is parked in: its middle, and how far round it counts as in it.
const BAY: [f32; 3] = [-2.75, -0.3, 2.6];
/// The foot of the hinge its room's door swings on: across, deep and up. The door is an actor
/// (`scenes/outback/shed_door.actor`), placed in the hub's text where the bake says.
const HINGE: [f32; 3] = [0.785, -0.37, 0.19];
/// The room's floor, a step up from the slab: its middle, its size, and how high it is.
const FLOOR: ([f32; 2], [f32; 2], f32) = ([2.5, 1.55], [4.9, 3.9], 0.17);
/// Its lanterns: across, deep and up.
const LANTERNS: [[f32; 3]; 2] = [[-2.5, 0.0, 2.38], [2.5, -3.5, 2.82]];

/// Where the shed stands.
#[derive(Clone, Copy, Debug)]
pub struct Shed {
    /// The middle of its slab, and the ground's height there.
    pub at: Vec2,
    pub height: f32,
    /// The way its open front faces: toward the road.
    front: Vec2,
}

impl Shed {
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

    /// How far a place is outside its slab: nothing, on it.
    fn outside(&self, x: f32, z: f32) -> f32 {
        let at = self.plan(x, z);
        let (low, high) = (Vec2::from(SLAB.0), Vec2::from(SLAB.1));
        (low - at).max(at - high).max(Vec2::ZERO).length()
    }

    /// Whether a place is on its slab, or within `margin` of it: nothing grows there.
    pub fn covers(&self, x: f32, z: f32, margin: f32) -> bool {
        self.outside(x, z) <= margin
    }

    /// How much the ground at a place is held to the slab's level: all of it under the shed
    /// and a pace round it, easing off to none.
    pub fn held(&self, x: f32, z: f32) -> f32 {
        let away = self.outside(x, z);
        1.0 - ((away - 0.6) / 2.6).clamp(0.0, 1.0)
    }

    /// The turn that faces it at the road, about the upright, in radians.
    fn yaw(&self) -> f32 {
        self.front.x.atan2(self.front.y)
    }

    /// The gate: a trailhead beside the shed, at its left-hand end, off the road. A track
    /// of its own leaves from it.
    pub fn gate(&self) -> Vec2 {
        self.place(SLAB.0[0] - 9.5, 0.5)
    }

    /// The bay a truck is parked in: where, and how far round.
    pub fn bay(&self) -> (Vec3, f32) {
        let at = self.place(BAY[0], BAY[1]);
        (Vec3::new(at.x, self.height, at.y), BAY[2])
    }

    /// Its walls, and what stands in it, as a truck meets them and as somebody on foot
    /// does; and the room's floor, to stand on.
    pub fn blockers(&self) -> Vec<Blocker> {
        let mut blockers: Vec<Blocker> = SOLID
            .iter()
            .map(|&[x, y, wide, deep]| {
                let at = self.place(x, y);
                Blocker {
                    centre: Vec3::new(at.x, self.height + 1.5, at.y),
                    size: Vec3::new(wide, 3.0, deep),
                    yaw_degrees: self.yaw().to_degrees(),
                }
            })
            .collect();
        let ([x, y], [wide, deep], high) = FLOOR;
        let at = self.place(x, y);
        blockers.push(Blocker {
            centre: Vec3::new(at.x, self.height + high * 0.5, at.y),
            size: Vec3::new(wide, high, deep),
            yaw_degrees: self.yaw().to_degrees(),
        });
        // The chest: its floor, and its four sides round what is in it.
        let ([x, y], [wide, deep], floor, rim) = CHEST;
        let (half_wide, half_deep) = ((wide - CHEST_SIDE) * 0.5, (deep - CHEST_SIDE) * 0.5);
        let mut boxed = |x: f32, y: f32, wide: f32, deep: f32, from: f32, to: f32| {
            let at = self.place(x, y);
            blockers.push(Blocker {
                centre: Vec3::new(at.x, self.height + (from + to) * 0.5, at.y),
                size: Vec3::new(wide, to - from, deep),
                yaw_degrees: self.yaw().to_degrees(),
            });
        };
        boxed(x, y, wide, deep, 0.0, floor);
        for side in [-1.0, 1.0] {
            boxed(x + side * half_wide, y, CHEST_SIDE, deep, floor, rim);
            boxed(
                x,
                y + side * half_deep,
                wide - 2.0 * CHEST_SIDE,
                CHEST_SIDE,
                floor,
                rim,
            );
        }
        blockers
    }

    /// Where its chest stands: the middle of its floor, inside, and how it is turned about
    /// the upright, in radians. Its length lies along its own z.
    pub fn chest(&self) -> (Vec3, f32) {
        let ([x, y], _, floor, _) = CHEST;
        let at = self.place(x, y);
        (Vec3::new(at.x, self.height + floor, at.y), self.yaw())
    }

    /// Where the room's door hangs, and the way it faces shut, as a yaw in degrees.
    pub fn door(&self) -> (Vec3, f32) {
        let at = self.place(HINGE[0], HINGE[1]);
        (
            Vec3::new(at.x, self.height + HINGE[2], at.y),
            self.yaw().to_degrees(),
        )
    }

    /// Puts the model in the scene, and its lanterns' light. `base` is the folder the scene's
    /// text is in.
    pub fn build(&self, scene: &mut SceneAsset, base: &Path) -> Result<(), String> {
        let path = base.join(MODEL);
        if !path.exists() {
            return Err(format!(
                "{} is missing: the shed is made by `blender -b --python tools/blender/shed/build.py -- target/outback/shed.glb` (bake.bat does it)",
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
        // What stops a truck is said in boxes: the model itself is only looked at, as is
        // everything the bush's design puts in a scene.
        append_model(
            scene,
            model,
            &ModelPlacements {
                whole: vec![transform],
                parts: Vec::new(),
            },
        )?;
        for [x, y, up] in LANTERNS {
            let at = self.place(x, y);
            scene.lights.push(Light {
                position: [at.x, self.height + up, at.y],
                range: 9.0,
                intensity: [2.4, 1.3, 0.5],
                _pad: 0,
            });
        }
        println!(
            "  the shed stands at {:.2} {:.2} {:.2}, its bay at {:.2} {:.2}",
            self.at.x,
            self.height,
            self.at.y,
            self.bay().0.x,
            self.bay().0.z
        );
        let (hinge, faces) = self.door();
        println!(
            "  its door hangs at `actor outback/shed_door.actor {:.2} {:.2} {:.2} {:.1}`",
            hinge.x, hinge.y, hinge.z, faces
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shed_faces_the_road_and_its_plan_maps_both_ways() {
        // A road running north (toward -z): the shed is to its west, facing east.
        let shed = Shed::beside(Vec2::new(10.0, 20.0), Vec2::new(0.0, -1.0), 5.0);
        assert!((shed.at - Vec2::new(10.0 - AWAY, 20.0)).length() < 1e-4);
        // In front of it is toward the road; its right, for somebody facing it, is north.
        assert!((shed.place(0.0, -AWAY) - Vec2::new(10.0, 20.0)).length() < 1e-4);
        assert!((shed.place(1.0, 0.0) - shed.at - Vec2::new(0.0, -1.0)).length() < 1e-4);
        let there = shed.place(2.0, -1.5);
        assert!((shed.plan(there.x, there.y) - Vec2::new(2.0, -1.5)).length() < 1e-4);
        // The model's front is its +z, which the turn carries to the way it faces.
        let front = Quat::from_rotation_y(shed.yaw()) * Vec3::Z;
        assert!((front - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-4);
        // Nothing grows on the slab, and the road is clear of it and of what is held level.
        assert!(shed.covers(shed.at.x, shed.at.y, 0.0));
        assert!(!shed.covers(10.0, 20.0, 2.5));
        assert_eq!(shed.held(10.0, 20.0), 0.0);
        assert_eq!(shed.held(shed.at.x, shed.at.y), 1.0);
        // The bay is inside the walls, and every wall is a box as tall as a truck.
        let (bay, round) = shed.bay();
        assert!(shed.covers(bay.x, bay.z, 0.0) && round > 2.0);
        assert!(shed.blockers().iter().filter(|b| b.size.y >= 3.0).count() == SOLID.len());
        // Its door hangs in the room's front wall, between the two parts of it.
        let hinge = shed.plan(shed.door().0.x, shed.door().0.z);
        assert!(hinge.x > 0.74 && hinge.x < 1.66 && (hinge.y + 0.4).abs() < 0.1);
    }
}
