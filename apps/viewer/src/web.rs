//! The browser front end: a small API for `web/index.html` to drive.
//!
//! JavaScript owns the page: the animation loop, input and UI. Rust owns the renderer.
//!
//! Built with the `gameplay` feature, it can also play a scene's game: the page passes on keys,
//! the mouse, gamepads and touch, and shows what scripts log. The plain build has none of that,
//! and only says whether a scene has a game to play, so the page can offer the other build.

use crate::{initial_camera, CameraInput, DayClock, FlyCamera, DEFAULT_DAY_SECONDS};
use glam::Vec3;
use solstice_asset::{file, GameFile};
use solstice_render::{
    DebugView, Effect, Gpu, Observation, Quality, Renderer, RendererSettings, StreamingBudget,
    TimeOfDay, Weather,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

fn error_string(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

/// Where the bytes of the file a game's maps share are numbered from, in the requests of a
/// map joined with it: a request from here on is that far into the shared file.
#[cfg(feature = "gameplay")]
#[wasm_bindgen]
pub fn shared_file_start() -> f64 {
    crate::shared::WEB_AT as f64
}

/// The most games that a browser keeps, and where each is kept in its storage: the first
/// where the one game there used to be was.
#[cfg(feature = "gameplay")]
const SLOTS: u32 = 3;
#[cfg(feature = "gameplay")]
fn slot_key(slot: u32) -> String {
    if slot <= 1 {
        "aussie-outback-4x4.profile".to_owned()
    } else {
        format!("aussie-outback-4x4.profile{slot}")
    }
}

/// The browser's storage for this site, if it lets it be used.
#[cfg(feature = "gameplay")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// The places there are for kept games: each its number, and what is kept in it, if a game
/// is.
#[cfg(feature = "gameplay")]
fn kept_games() -> Vec<(u32, Option<String>)> {
    let storage = storage();
    (1..=SLOTS)
        .map(|slot| {
            let kept = storage
                .as_ref()
                .and_then(|storage| storage.get_item(&slot_key(slot)).ok().flatten());
            (slot, kept)
        })
        .collect()
}

/// Keeps a game in its place.
#[cfg(feature = "gameplay")]
fn keep_game(slot: u32, kept: &str) {
    if let (true, Some(storage)) = ((1..=SLOTS).contains(&slot), storage()) {
        if storage.set_item(&slot_key(slot), kept).is_err() {
            log::warn!(
                "the game in place {slot} could not be kept: the browser's storage is full or shut"
            );
        }
    }
}

/// One map of a game, before its scene is read: the game round the driving, taken up where
/// the page before this one left it, and the packs of the trucks the scene is to be read
/// with. A browser reads a page for each map. The page makes this first, fetches the packs
/// it asks for, and gives it to [`WebViewer::create`] with them.
#[cfg(feature = "gameplay")]
#[wasm_bindgen]
pub struct WebGame {
    shell: crate::shell::Shell,
    packs: Vec<(String, Vec<u8>)>,
}

#[cfg(feature = "gameplay")]
#[wasm_bindgen]
impl WebGame {
    /// `world` is the game's maps (`shell::World`) and `carried` what the page before this
    /// one wrote down as it left (`WebViewer::ui_carry`), or nothing. With nothing carried
    /// the game is at its title, in its hub; or, if the address says so, `map` is being
    /// driven for `mode`, as a free drive. Rejects with what is wrong in `world`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        world: &str,
        carried: &str,
        map: &str,
        mode: &str,
        driving: bool,
    ) -> Result<WebGame, JsValue> {
        use crate::shell::Shell;
        let world = crate::shell::World::parse(world).map_err(|e| JsValue::from_str(&e))?;
        let mut shell = if carried.is_empty() {
            let mut shell = Shell::new(world, "");
            if !map.is_empty() {
                shell.start_in(map);
            }
            if driving {
                shell.resume(mode);
            }
            shell
        } else {
            // A kept game is as it was last kept, which may be since that page left; one
            // that is kept nowhere is as that page had it.
            let words = |line: &str| line.strip_prefix("save ").and_then(|n| n.parse().ok());
            let save: u32 = carried.lines().find_map(words).unwrap_or(0);
            let kept = kept_games()
                .into_iter()
                .find(|(slot, _)| *slot == save)
                .and_then(|(_, kept)| kept)
                .or_else(|| Shell::carried_kept(carried).map(str::to_owned))
                .unwrap_or_default();
            let mut shell = Shell::new(world, &kept);
            shell.take_up(carried);
            shell
        };
        shell.set_web();
        let playing = shell.playing();
        shell.set_saves(&kept_games(), playing);
        Ok(WebGame {
            shell,
            packs: Vec::new(),
        })
    }

    /// The trucks whose packs the scene is to be read with, by their names: each is
    /// `trucks/NAME.truck` beside the maps.
    pub fn packs_wanted(&self) -> Vec<String> {
        self.shell.packs()
    }

    /// Gives one of those packs, as it was fetched.
    pub fn give_pack(&mut self, name: &str, bytes: Vec<u8>) {
        self.packs.push((name.to_owned(), bytes));
    }
}

#[cfg(feature = "gameplay")]
impl WebGame {
    /// Puts in a scene the trucks it is to be played with, each put together from its pack
    /// and kept in `memory`: as `put_trucks` does for a scene on a disk.
    fn put_trucks(
        &mut self,
        scene: &mut solstice_asset::PagedScene,
        memory: &mut crate::shared::Memory,
    ) -> Result<(), String> {
        use crate::shared::{self, Streamed};
        let packs = std::mem::take(&mut self.packs);
        let assemble = |name: &str, build: &str, pull: f32| {
            let (_, bytes) = packs
                .iter()
                .find(|(named, _)| named == name)
                .ok_or_else(|| format!("the pack of the truck {name} was not fetched"))?;
            crate::kit::assemble(&mut std::io::Cursor::new(&bytes[..]), Some(build), pull)
                .map_err(|error| format!("{name}: {error}"))
        };
        // What the truck that is driven is fitted with is said by its kit, which is in
        // its pack.
        if let Some(driven) = self.shell.driven() {
            if let Some((_, bytes)) = packs.iter().find(|(named, _)| *named == driven) {
                if let Ok(kit) = crate::kit::pack_kit(&mut std::io::Cursor::new(&bytes[..])) {
                    self.shell.know_kit(&driven, kit);
                }
            }
        }
        let mut joins = Streamed { scene, memory };
        let shown = self.shell.showing();
        let pull = self.shell.power();
        let wearing = self.shell.wearing();
        if !shown.is_empty() {
            let mut trucks = Vec::new();
            for (truck, build) in &shown {
                trucks.push(assemble(truck, build, 1.0)?);
            }
            shared::show_trucks(&mut joins, trucks, self.shell.stand())?;
        } else if let Some((truck, fit)) = wearing {
            shared::swap_truck(&mut joins, assemble(&truck, &fit, pull)?)?;
        }
        let spots = shared::lot(&joins.scene.gameplay);
        let (parked, pulled) = self.shell.parked();
        if !spots.is_empty() && !parked.is_empty() {
            let mut trucks = Vec::new();
            for sale in parked {
                trucks.push(shared::Parked {
                    place: sale.place,
                    truck: assemble(&sale.truck, "stock", 1.0)?,
                    paint: sale.paint,
                    price: sale.price,
                    km: sale.km,
                });
            }
            shared::park_trucks(&mut joins, trucks, &spots, pulled)?;
        }
        Ok(())
    }
}

/// A game, which the build that only looks at scenes knows nothing of: see the other.
#[cfg(not(feature = "gameplay"))]
#[wasm_bindgen]
pub struct WebGame {}

/// Sets up logging and panic messages. Runs once when the module loads.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
}

/// A running viewer, bound to one canvas.
#[wasm_bindgen]
pub struct WebViewer {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    camera: FlyCamera,
    /// The time of day, which places the sun and moon.
    clock: DayClock,
    gpu_errors: Arc<AtomicU32>,
    /// The scene's gameplay files, empty if it has none.
    gameplay: Vec<GameFile>,
    /// Where its collision shapes are, and those delivered so far, inflated.
    #[cfg(feature = "gameplay")]
    collision: Option<solstice_asset::CollisionIndex>,
    #[cfg(feature = "gameplay")]
    collision_data_offset: u64,
    #[cfg(feature = "gameplay")]
    collision_pieces: Vec<Vec<u8>>,
    /// What animated actors need from the scene.
    #[cfg(feature = "gameplay")]
    skeletons: Vec<solstice_asset::Skeleton>,
    #[cfg(feature = "gameplay")]
    clips: Vec<solstice_asset::Clip>,
    #[cfg(feature = "gameplay")]
    poses: Vec<u32>,
    #[cfg(feature = "gameplay")]
    instance_poses: Vec<u32>,
    #[cfg(feature = "gameplay")]
    play: Option<crate::play::PlayMode>,
    /// The game round the driving, if the scene is one map of a game (`ui_start`), what it
    /// last had to tell its pages, and what it has asked of the page that holds them.
    #[cfg(feature = "gameplay")]
    shell: Option<crate::shell::Shell>,
    /// The game the scene was made for, until it is started (`ui_start`); which of the
    /// kept games is being played; and the trucks that were joined onto the scene from
    /// their packs, whose bytes are asked for as the scene's are and answered from here.
    #[cfg(feature = "gameplay")]
    game: Option<crate::shell::Shell>,
    #[cfg(feature = "gameplay")]
    playing: u32,
    #[cfg(feature = "gameplay")]
    memory: crate::shared::Memory,
    #[cfg(feature = "gameplay")]
    told: String,
    #[cfg(feature = "gameplay")]
    orders: Vec<String>,
    /// See [`WebViewer::external_tables`].
    external_tables: Option<Vec<f64>>,
    pinned_sizes: Vec<u32>,
    pinned_clusters: Vec<u32>,
    /// What scripts logged, and errors, for the page to take.
    #[cfg(feature = "gameplay")]
    messages: Vec<crate::play::Message>,
    /// Which entity of the scene's project made which instances, for the editor.
    #[cfg(feature = "editor")]
    sources: Vec<solstice_asset::Source>,
}

/// How far around the camera a scene read in parts loads what isn't in view, in metres.
const LARGE_SCENE_PREFETCH: f32 = 24.0;
/// Such a scene's clusters under this many texels of a shadow cascade, or voxels of the GI,
/// across are drawn into it as specks: its coarsest clusters are small, and far cascades and
/// coarse voxels are nearly all specks.
const LARGE_SCENE_SPECKS: f32 = 1.0;

#[wasm_bindgen]
impl WebViewer {
    /// Creates a viewer from an unpacked manifest of a scene, which draws what it has while
    /// `requests` asks for more. `data_offset` is where the file's data area begins. Budgets
    /// are in bytes of GPU memory. Rejects if WebGPU is unavailable or the GPU lacks required
    /// capabilities, with a message suitable for showing to the user. `unpack_textures` acts as
    /// if the GPU had no block compression, to test that path. `default_limits` asks for no more
    /// than the WebGPU default limits, as many phones are given, to see their fallbacks on any
    /// GPU.
    ///
    /// `shared` is the unpacked manifest of the file a game's maps share, if the page has
    /// one, and `shared_data_offset` where that file's data area begins. A map that is read
    /// with it is joined with it here (`crate::shared`), and asks for the shared file's bytes
    /// as ranges from [`shared_file_start`] on. A scene that is not takes no notice of it.
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        canvas: HtmlCanvasElement,
        manifest: Vec<u8>,
        data_offset: f64,
        geometry_budget: f64,
        texture_budget: f64,
        unpack_textures: bool,
        default_limits: bool,
        shared: Option<Vec<u8>>,
        shared_data_offset: Option<f64>,
        game: Option<WebGame>,
    ) -> Result<WebViewer, JsValue> {
        #[allow(unused_mut)]
        let mut scene =
            file::decode_manifest(&manifest, data_offset as u64).map_err(error_string)?;
        drop(manifest);
        #[cfg(feature = "gameplay")]
        if crate::shared::wanted(&scene.gameplay) {
            let bytes = shared.ok_or_else(|| {
                error_string(format!(
                    "this map is read with {}, which could not be read",
                    crate::shared::FILE
                ))
            })?;
            let other = file::decode_manifest(&bytes, shared_data_offset.unwrap_or(0.0) as u64)
                .map_err(error_string)?;
            drop(bytes);
            let files = other.gameplay.clone();
            let joined = solstice_asset::join::join(&mut scene, other, crate::shared::WEB_AT)
                .map_err(error_string)?;
            crate::shared::adopt(&mut scene, &files, joined).map_err(error_string)?;
            // And what nuggets are shown with, which are made as they are found.
            crate::nugget::join_streamed(&mut scene).map_err(error_string)?;
        }
        // A game's trucks, put together from their packs: the one that is driven, or
        // those that stand to be looked at, and those parked in the used-truck yard.
        #[cfg(feature = "gameplay")]
        let mut memory = crate::shared::Memory::default();
        #[cfg(feature = "gameplay")]
        let game = match game {
            Some(mut game) => {
                game.put_trucks(&mut scene, &mut memory)
                    .map_err(error_string)?;
                Some(game.shell)
            }
            None => None,
        };
        #[cfg(not(feature = "gameplay"))]
        let _ = (shared, shared_data_offset, game);
        let external_tables = scene
            .external
            .map(|tables| vec![tables.clusters as f64, tables.pinned_bytes as f64]);
        let pinned_clusters: Vec<u32> = scene
            .external
            .map(|_| {
                (0..scene.pinned_pages)
                    .map(|page| scene.page_cluster_count(page))
                    .collect()
            })
            .unwrap_or_default();
        let pinned_sizes: Vec<u32> = scene
            .external
            .map(|_| {
                scene.pages[..scene.pinned_pages as usize]
                    .iter()
                    .map(|page| page.raw_bytes() as u32)
                    .collect()
            })
            .unwrap_or_default();

        let (width, height) = (canvas.width().max(1), canvas.height().max(1));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(error_string)?;
        let defaults = wgpu::Limits::defaults();
        let mut gpu = Gpu::request_within(
            &instance,
            Some(&surface),
            wgpu::PowerPreference::HighPerformance,
            default_limits.then_some(&defaults),
        )
        .await
        .map_err(error_string)?;
        gpu.block_compressed_textures &= !unpack_textures;

        let gpu_errors = Arc::new(AtomicU32::new(0));
        let counter = gpu_errors.clone();
        gpu.device
            .on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
                counter.fetch_add(1, Ordering::Relaxed);
                log::error!("GPU error: {error}");
            }));

        let mut config = surface
            .get_default_config(&gpu.adapter, width, height)
            .ok_or_else(|| JsValue::from_str("this canvas cannot be presented to"))?;
        config.format = config.format.remove_srgb_suffix();
        surface.configure(&gpu.device, &config);

        let budget = StreamingBudget {
            geometry_bytes: geometry_budget as u64,
            texture_bytes: texture_budget as u64,
        };
        // A scene too large for one binding needs more storage buffers than some GPUs have.
        if let Some(reason) = Renderer::streaming_unsupported(&gpu, &scene, budget) {
            return Err(JsValue::from_str(&reason));
        }
        let renderer = Renderer::streaming(
            &gpu,
            &scene,
            config.format,
            RendererSettings {
                // A scene this large takes a moment to bring in what a turn reveals.
                prefetch_radius: if scene.external.is_some() {
                    LARGE_SCENE_PREFETCH
                } else {
                    0.0
                },
                speck_texels: if scene.external.is_some() {
                    LARGE_SCENE_SPECKS
                } else {
                    0.0
                },
                ..Default::default()
            },
            budget,
        );
        Ok(WebViewer {
            gpu,
            surface,
            config,
            renderer,
            camera: FlyCamera::new(initial_camera(scene.viewpoint)),
            clock: DayClock::new(TimeOfDay::default(), DEFAULT_DAY_SECONDS),
            gpu_errors,
            external_tables,
            pinned_sizes,
            pinned_clusters,
            gameplay: scene.gameplay,
            #[cfg(feature = "gameplay")]
            collision: scene.collision,
            #[cfg(feature = "gameplay")]
            collision_data_offset: scene.data_offset,
            #[cfg(feature = "gameplay")]
            collision_pieces: Vec::new(),
            #[cfg(feature = "gameplay")]
            skeletons: scene.skeletons,
            #[cfg(feature = "gameplay")]
            clips: scene.clips,
            #[cfg(feature = "gameplay")]
            poses: scene.poses,
            #[cfg(feature = "gameplay")]
            instance_poses: scene.instance_poses,
            #[cfg(feature = "gameplay")]
            play: None,
            #[cfg(feature = "gameplay")]
            shell: None,
            #[cfg(feature = "gameplay")]
            playing: game.as_ref().map_or(0, crate::shell::Shell::playing),
            #[cfg(feature = "gameplay")]
            game,
            #[cfg(feature = "gameplay")]
            memory,
            #[cfg(feature = "gameplay")]
            told: String::new(),
            #[cfg(feature = "gameplay")]
            orders: Vec::new(),
            #[cfg(feature = "gameplay")]
            messages: Vec::new(),
            #[cfg(feature = "editor")]
            sources: scene.sources,
        })
    }

    /// For a scene whose tables come separately: how many cluster records and how many bytes
    /// of pinned pages. Empty for any other scene.
    pub fn external_tables(&self) -> Vec<f64> {
        self.external_tables.clone().unwrap_or_default()
    }

    /// The size in bytes of each pinned page of such a scene, in order.
    pub fn pinned_sizes(&self) -> Vec<u32> {
        self.pinned_sizes.clone()
    }

    /// How many cluster records each pinned page of such a scene has, in order. Their records
    /// are the first of the table, in this order.
    pub fn pinned_cluster_counts(&self) -> Vec<u32> {
        self.pinned_clusters.clone()
    }

    /// Takes one of such a scene's pinned pages and its cluster records, in order.
    pub fn install_pinned(&mut self, page: u32, bytes: &[u8], clusters: &[u8]) {
        self.renderer
            .install_pinned(&self.gpu, page, bytes, clusters);
    }

    /// Takes a page of such a scene, inflated, with its cluster records.
    pub fn deliver_page(&mut self, id: u32, bytes: Vec<u8>, clusters: Vec<u8>) -> bool {
        self.renderer.deliver_fetched_page(id, bytes, clusters)
    }

    /// Call when all of such a scene's tables have been handed over.
    pub fn finish_external_tables(&mut self) {
        self.renderer.finish_external_tables(&self.gpu);
    }

    /// What to fetch next, as a flat array with six numbers per request: its id, the first
    /// byte in the file, one past the last byte, 1 if it must be inflated, and the range of its
    /// cluster records in their table, which is empty unless they must come too. Each request
    /// must later go to `deliver`, `deliver_page` or `request_failed`.
    pub fn stream_requests(&mut self, max_fetching: u32) -> Vec<f64> {
        #[allow(unused_mut)]
        let mut requests = self.renderer.stream_requests(max_fetching).to_vec();
        // What is in memory is handed over at once: a truck put together from its pack has
        // no file for the page to fetch from.
        #[cfg(feature = "gameplay")]
        requests.retain(|request| {
            let Some(bytes) = self.memory.read(request.start, request.end) else {
                return true;
            };
            let data = if request.deflated {
                file::inflate(bytes).ok()
            } else {
                Some(bytes.to_vec())
            };
            match data {
                Some(data) => {
                    self.renderer.deliver_fetched(request.id, data);
                }
                None => self.renderer.request_failed(request.id),
            }
            false
        });
        requests
            .iter()
            .flat_map(|request| {
                [
                    f64::from(request.id),
                    request.start as f64,
                    request.end as f64,
                    f64::from(u8::from(request.deflated)),
                    request.clusters.map_or(0.0, |range| range.0 as f64),
                    request.clusters.map_or(0.0, |range| range.1 as f64),
                ]
            })
            .collect()
    }

    /// Takes the bytes of a request, inflated if it needed that.
    pub fn deliver(&mut self, id: u32, bytes: Vec<u8>) -> bool {
        self.renderer.deliver_fetched(id, bytes)
    }

    /// Reports that a request could not be fetched.
    pub fn request_failed(&mut self, id: u32) {
        self.renderer.request_failed(id);
    }

    /// Geometry pages on the GPU.
    pub fn pages_installed(&self) -> u32 {
        self.renderer.stats().streaming.installed
    }

    pub fn pages_total(&self) -> u32 {
        self.renderer.stats().streaming.pages
    }

    /// Size of the page pool, in pages.
    pub fn page_slots(&self) -> u32 {
        self.renderer.stats().streaming.slots
    }

    pub fn pages_evicted(&self) -> f64 {
        self.renderer.stats().streaming.evictions as f64
    }

    /// Wanted pages with no room in the pool.
    pub fn pages_starved(&self) -> u32 {
        self.renderer.stats().streaming.starved
    }

    /// What the shadow cascade last redrawn drew: clusters, triangles, and clusters that found
    /// no room in its lists, which leave holes in the shadows.
    pub fn shadow_load(&self) -> Vec<u32> {
        let stats = self.renderer.stats();
        vec![
            stats.shadow_clusters,
            stats.shadow_triangles,
            stats.shadow_dropped,
        ]
    }

    /// For a scene with several pools: the pages in each and its size, a pair per pool.
    pub fn page_pools(&self) -> Vec<u32> {
        let bands = self.renderer.stats().streaming.bands;
        bands
            .iter()
            .flatten()
            .flat_map(|&(installed, slots)| [installed, slots])
            .collect()
    }

    /// Layers sharpened beyond the base tier, as middle and top counts.
    pub fn texture_layers_sharp(&self) -> Vec<u32> {
        let streaming = self.renderer.stats().streaming;
        vec![streaming.middle_layers, streaming.top_layers]
    }

    /// Bytes of GPU memory for this scene: records, page pool, then textures.
    pub fn scene_memory(&self) -> Vec<f64> {
        let memory = self.renderer.scene_bytes();
        vec![
            memory.records as f64,
            memory.pages as f64,
            memory.textures as f64,
        ]
    }

    /// Updates the canvas size. Pass physical pixels.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.gpu.device, &self.config);
        }
    }

    /// Advances the camera and draws one frame.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        delta_seconds: f32,
        forward: f32,
        right: f32,
        up: f32,
        look_yaw: f32,
        look_pitch: f32,
        fast: bool,
    ) {
        if !self.playing_as_someone() {
            self.camera.update(
                &CameraInput {
                    forward,
                    right,
                    up,
                    look_yaw,
                    look_pitch,
                    fast,
                },
                delta_seconds,
            );
        }
        let environment = self.clock.tick(delta_seconds);
        #[cfg(feature = "gameplay")]
        let environment = crate::night_eyes(environment, self.shell.is_some());
        #[cfg(feature = "gameplay")]
        let mut saves = Vec::new();
        #[cfg(feature = "gameplay")]
        if let Some(play) = &mut self.play {
            use crate::shell::{Order, Step};
            // By dark, measured before eyes are opened to it, the lamps are on.
            play.lamps = crate::is_dark(&self.clock.environment());
            let step = match &mut self.shell {
                Some(shell) => shell.before(&mut play.devices, delta_seconds),
                None => Step::Play,
            };
            let camera = &mut self.camera.camera;
            let messages = match step {
                Step::Play => play.frame(&mut self.renderer, camera, delta_seconds),
                Step::Unattended(orbit) => {
                    play.frame_unattended(&mut self.renderer, camera, delta_seconds, orbit)
                }
                Step::Held => {
                    play.frame_held(&self.renderer, delta_seconds);
                    Vec::new()
                }
            };
            if let Some(shell) = &mut self.shell {
                shell.see(play.things());
                shell.after(play.play.hud());
                shell.bound(play.play.input_map());
                shell.see_dirt(play.dirt());
                let blows = play.take_damage_changes();
                if !blows.is_empty() {
                    shell.see_damage(play.damage().to_text(), &blows);
                }
                // While somebody else's run is watched, the camera is the shot's.
                if let Some((position, yaw, pitch)) = shell.view() {
                    (camera.position, camera.yaw, camera.pitch) = (position, yaw, pitch);
                }
                let stands = play.play.car_pose();
                if let Some((at, turned, seconds)) = shell.showroom() {
                    let (at, turned) = play.stand().unwrap_or((at, turned));
                    (camera.position, camera.yaw, camera.pitch) =
                        play.showroom_camera(at, turned, seconds, shell.focus());
                }
                let preview = shell.preview();
                play.set_unseen(&mut self.renderer, preview.is_some());
                if let Some((at, seconds)) = preview {
                    let (at, turned) = stands.unwrap_or((at, glam::Quat::IDENTITY));
                    (camera.position, camera.yaw, camera.pitch) =
                        play.preview_camera(at, turned, seconds);
                }
                play.set_detector(shell.detector());
                if let Some(told) = shell.tell(play.play.hud(), delta_seconds) {
                    self.told = told;
                }
                let now = self.clock.time.hours;
                shell.set_hour(now);
                for order in shell.take_orders() {
                    self.orders.push(match order {
                        // The desktop's: in a browser the page takes hold of the mouse itself.
                        Order::Look => continue,
                        // A night slept through: it is the next morning.
                        Order::Sleep { hours, days } => {
                            self.clock.time.hours = hours;
                            self.clock.time.day += days as f32;
                            continue;
                        }
                        Order::Item(shown) => {
                            play.show_item(&mut self.renderer, shown);
                            continue;
                        }
                        Order::Thing(body) => {
                            play.thing(body);
                            continue;
                        }
                        Order::Carry(carried) => {
                            play.set_carried(carried);
                            continue;
                        }
                        Order::Sound(name) => {
                            play.sound_once(name);
                            continue;
                        }
                        Order::Door { piece, open } => {
                            let _ = play.swing(&piece, if open { 1.0 } else { 0.0 });
                            continue;
                        }
                        Order::Paint(colour) => {
                            play.paint(colour);
                            continue;
                        }
                        Order::Dirt(dirt) => {
                            play.set_dirt(dirt);
                            continue;
                        }
                        Order::Damage(damage) => {
                            let _ = play.set_damage(&damage);
                            continue;
                        }
                        Order::Saves(what) => {
                            saves.push(what);
                            continue;
                        }
                        // (A browser's page is as loud and as big as the browser has it.)
                        Order::Options => continue,
                        Order::Show { index, paint } => {
                            play.display(&mut self.renderer, index, paint);
                            continue;
                        }
                        Order::Unpark(place) => {
                            play.unpark(&mut self.renderer, place);
                            continue;
                        }
                        Order::Fit(build) => {
                            let _ = play.display_fit(&build);
                            continue;
                        }
                        Order::Afoot(at) => {
                            play.play.stand_at(at);
                            continue;
                        }
                        Order::Swap => {
                            if play.play.afoot() {
                                play.play.get_in(crate::shell::GET_IN + 1.0);
                            } else {
                                play.play.get_out();
                            }
                            continue;
                        }
                        Order::Place { position, yaw } => {
                            play.play.place_car(position, yaw);
                            continue;
                        }
                        // A timed run, or somebody else's being watched.
                        Order::Stage(stage) => {
                            crate::cinema::stage(play, stage);
                            continue;
                        }
                        Order::Quit => "quit".to_owned(),
                        Order::Load { map, scene, sky } => format!(
                            "load {map} {scene} {} {}",
                            shell.mode_name(),
                            sky.map_or("-".to_owned(), |(hours, weather, soaked)| {
                                // (No hour is the hour it is: the clock goes on.)
                                let hours = if hours < 0.0 { now } else { hours };
                                format!("{hours} {weather} {soaked}")
                            })
                        ),
                    });
                }
            }
            self.messages.extend(messages);
            // Kept for the page to take; a page that never does still holds only the newest.
            let excess = self
                .messages
                .len()
                .saturating_sub(solstice_game::play::MAX_MESSAGES);
            self.messages.drain(..excess);
        }
        #[cfg(feature = "gameplay")]
        for what in saves {
            self.change_game(what);
        }
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                return;
            }
            _ => return,
        };
        let view = texture.texture.create_view(&Default::default());
        self.renderer.render(
            &self.gpu,
            &self.camera.camera,
            &environment,
            &view,
            (self.config.width, self.config.height),
            delta_seconds,
        );
        self.gpu.queue.present(texture);
    }

    /// Selects a debug view by name. Returns false if there is no such view.
    pub fn set_debug_view(&mut self, name: &str) -> bool {
        match DebugView::from_name(name) {
            Some(view) => {
                self.renderer.settings_mut().debug_view = view;
                true
            }
            None => false,
        }
    }

    /// Applies a quality preset by name. Returns false if there is no such preset.
    pub fn set_quality(&mut self, name: &str) -> bool {
        match Quality::from_name(name) {
            Some(quality) => {
                self.renderer.settings_mut().apply_quality(quality);
                true
            }
            None => false,
        }
    }

    /// Sets the time of day, in hours from 0 to 24: noon is 12.
    pub fn set_time(&mut self, hours: f32) {
        self.clock.time.hours = hours.rem_euclid(24.0);
    }

    /// The time of day now, in hours, for the page to show as it passes.
    pub fn time_of_day(&self) -> f32 {
        self.clock.time.hours
    }

    /// How many real seconds a whole day takes to pass, or 0 to hold the time still.
    pub fn set_day_length(&mut self, seconds: f32) {
        self.clock.day_seconds = seconds.max(0.0);
    }

    /// Turns the scene to face north at `degrees` clockwise from its -Z axis, which moves the
    /// sun, moon and stars around it together.
    pub fn set_north(&mut self, degrees: f32) {
        self.clock.place.north = -degrees.to_radians();
    }

    /// Thunder from strikes since the last call, for the page to sound, as JSON: a list of
    /// `{delay, distance, loudness, rumble, ground}`, seconds until it is heard, metres off,
    /// 0 to 1, seconds it rolls, and whether the bolt reached the ground.
    pub fn take_thunder(&mut self) -> String {
        let parts: Vec<String> = self
            .renderer
            .take_thunder()
            .iter()
            .map(|t| {
                format!(
                    "{{\"delay\":{:.3},\"distance\":{:.0},\"loudness\":{:.3},\"rumble\":{:.2},\"ground\":{}}}",
                    t.delay, t.distance, t.loudness, t.rumble, t.to_ground
                )
            })
            .collect();
        format!("[{}]", parts.join(","))
    }

    /// Makes lightning strike ahead on the next frame, whatever the weather.
    pub fn strike_lightning(&mut self) {
        self.renderer.strike_lightning();
    }

    /// Names of the built-in weather, separated by commas.
    pub fn weather_names() -> String {
        Weather::names().collect::<Vec<_>>().join(",")
    }

    /// Lets the weather change by itself, starting from the named weather, or stops it.
    pub fn set_weather_changing(&mut self, changing: bool, start: &str) {
        self.renderer.set_weather_changing(changing, start);
    }

    /// Changes to a built-in weather by name, easing into it over `seconds`. Returns false if
    /// there is no such weather.
    pub fn set_weather(&mut self, name: &str, seconds: f32) -> bool {
        match Weather::by_name(name) {
            Some(weather) => {
                self.renderer.set_weather(weather, seconds);
                true
            }
            None => false,
        }
    }

    /// Changes to the weather in a live report, easing into it over `seconds`: cloud cover low,
    /// middle and high in percent, wind near the ground in km/h and the compass direction it
    /// blows from, the WMO weather code, and the last hour's precipitation in millimetres.
    #[allow(clippy::too_many_arguments)]
    pub fn set_observed_weather(
        &mut self,
        cover_low: f32,
        cover_mid: f32,
        cover_high: f32,
        wind_kmh: f32,
        wind_from: f32,
        code: u32,
        precipitation: f32,
        seconds: f32,
    ) {
        let report = Observation {
            cover_low,
            cover_mid,
            cover_high,
            wind_kmh,
            wind_from,
            code,
            precipitation,
        };
        self.renderer
            .set_weather(Weather::observed(&report), seconds);
    }

    /// The weather now, briefly, for the page to show.
    pub fn weather_summary(&self) -> String {
        let w = self.renderer.weather();
        format!(
            "{:.0}% cloud, wind {:.0} m/s from {:.0}°{}",
            w.coverage * 100.0,
            w.wind_speed,
            w.wind_from,
            if w.rain > 0.05 { ", rain" } else { "" }
        )
    }

    /// Lays still water over the scene: ponds written `x0,z0,x1,z1,level`, with `;` between
    /// them, in place of the scene's own. Returns false, changing nothing, if they can't be
    /// read. Naming none, as `?ponds=;` does, keeps the scene's own, as the native viewer does.
    pub fn set_ponds(&mut self, ponds: &str) -> bool {
        match solstice_render::Pond::parse_list(ponds) {
            Ok(ponds) => {
                if !ponds.is_empty() {
                    self.renderer.set_ponds(&ponds);
                }
                true
            }
            Err(error) => {
                log::warn!("ponds: {error}");
                false
            }
        }
    }

    pub fn set_camera(&mut self, x: f32, y: f32, z: f32, yaw: f32, pitch: f32) {
        self.camera.camera.position = Vec3::new(x, y, z);
        self.camera.camera.yaw = yaw;
        self.camera.camera.pitch = pitch;
    }

    /// Where the camera is: x, y, z, yaw and pitch, as `set_camera` takes them.
    pub fn camera(&self) -> Vec<f32> {
        let camera = &self.camera.camera;
        let at = camera.position;
        vec![at.x, at.y, at.z, camera.yaw, camera.pitch]
    }

    pub fn triangles(&self) -> u32 {
        self.renderer.stats().triangles
    }

    pub fn clusters(&self) -> u32 {
        self.renderer.stats().clusters
    }

    pub fn software_clusters(&self) -> u32 {
        self.renderer.stats().software_clusters
    }

    pub fn instances(&self) -> u32 {
        self.renderer.stats().instances
    }

    /// Triangles if every instance rendered its finest LOD. A double, since it can exceed 32 bits.
    pub fn source_triangles(&self) -> f64 {
        self.renderer.stats().source_triangles as f64
    }

    /// How the scene's textures are held on the GPU.
    pub fn texture_summary(&self) -> String {
        self.renderer.texture_summary().to_owned()
    }

    /// Starts a built-in particle effect by name, standing at (x, y, z). Returns false if there
    /// is no such effect or no room for it.
    pub fn spawn_effect(&mut self, name: &str, x: f32, y: f32, z: f32) -> bool {
        let Some(effect) = Effect::by_name(name) else {
            return false;
        };
        let at = glam::Mat4::from_translation(Vec3::new(x, y, z));
        self.renderer.particles().spawn(&effect, at).is_some()
    }

    /// Particles alive, from a recent frame.
    pub fn particles_alive(&self) -> u32 {
        self.renderer.stats().particles.alive
    }

    /// What the renderer does differently for this GPU's limits, for the page to show: empty
    /// on a GPU with all it asks for.
    pub fn fallbacks(&self) -> String {
        let fallbacks = self.gpu.fallbacks;
        [
            (fallbacks.separate_depth, "depth apart"),
            (fallbacks.ocean_depth_from_pyramid, "sea depth from pyramid"),
            (fallbacks.pool_in_one_binding, "one-binding pool"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, name)| *name)
        .collect::<Vec<_>>()
        .join(", ")
    }

    pub fn gpu_error_count(&self) -> u32 {
        self.gpu_errors.load(Ordering::Relaxed)
    }

    /// Whether the scene has a game to play.
    pub fn has_gameplay(&self) -> bool {
        !self.gameplay.is_empty()
    }

    /// Forgets the scene's game, so that another can be given in its place.
    pub fn clear_game(&mut self) {
        self.gameplay.clear();
    }

    /// Gives the scene a gameplay file, in place of any of that name. So a game kept in a
    /// folder beside the page is played on a scene baked without it. Before `play`.
    pub fn add_game_file(&mut self, name: &str, text: &str) {
        self.gameplay.retain(|file| file.name != name);
        self.gameplay.push(GameFile {
            name: name.to_owned(),
            text: text.to_owned(),
        });
    }

    /// Whether this build can play one: the gameplay build can, the plain one can't.
    pub fn can_play() -> bool {
        cfg!(feature = "gameplay")
    }

    fn playing_as_someone(&self) -> bool {
        #[cfg(feature = "gameplay")]
        if let Some(play) = &self.play {
            return play.play.has_camera();
        }
        false
    }
}

/// Play mode, in the gameplay build only.
#[cfg(feature = "gameplay")]
#[wasm_bindgen]
impl WebViewer {
    /// Whether the scene was baked with collision, though everything in it may be `collide=none`.
    pub fn has_collision(&self) -> bool {
        self.collision.is_some()
    }

    /// The parts of the file holding the scene's collision shapes, fetched only to play: a flat
    /// array of two numbers each, the first byte and one past the last. Each is deflated. Empty
    /// for a scene with none.
    pub fn collision_ranges(&self) -> Vec<f64> {
        let Some(index) = &self.collision else {
            return Vec::new();
        };
        index
            .ranges()
            .iter()
            .flat_map(|&(offset, stored)| {
                let start = self.collision_data_offset + offset;
                [start as f64, (start + u64::from(stored)) as f64]
            })
            .collect()
    }

    /// The size each part of the collision unpacks to: no more should ever be unpacked.
    pub fn collision_sizes(&self) -> Vec<f64> {
        self.collision.as_ref().map_or_else(Vec::new, |index| {
            index.raw_sizes().iter().map(|&s| s as f64).collect()
        })
    }

    /// Takes one part of the collision, inflated, in the order of `collision_ranges`.
    pub fn deliver_collision(&mut self, bytes: Vec<u8>) {
        self.collision_pieces.push(bytes);
    }

    /// The files the scene's game names and lacks: for a game given file by file, what to give
    /// next. Empty once it has them all.
    pub fn game_files_missing(&self) -> Vec<String> {
        solstice_game::missing_files(&self.gameplay)
    }

    /// Starts the scene's game, with its collision delivered. Rejects with every mistake in its
    /// files, one to a line.
    pub fn play(&mut self) -> Result<(), JsValue> {
        if self.gameplay.is_empty() {
            return Err(JsValue::from_str("this scene has no game to play"));
        }
        let pieces = std::mem::take(&mut self.collision_pieces);
        let collision = match &self.collision {
            Some(index) => Some(index.decode(&pieces).map_err(error_string)?),
            None => None,
        };
        let animation = solstice_game::animation::AnimationAssets {
            skeletons: &self.skeletons,
            clips: &self.clips,
            poses: &self.poses,
            instance_poses: &self.instance_poses,
        };
        let play = crate::play::PlayMode::start(
            &mut self.renderer,
            &self.gameplay,
            collision.as_ref(),
            &animation,
        )
        .map_err(|errors| JsValue::from_str(&errors.join("\n")))?;
        self.play = Some(play);
        Ok(())
    }

    /// Makes the scene being played one map of a game: `world` is the game's maps
    /// (`shell::World`), `kept` what it kept from before (its best laps), and `map` which of
    /// them this scene is, the hub if empty. `driving` takes the game up where another page
    /// left it, driving this map for `mode` (`roam` or `laps`), rather than at its title.
    /// Call after `play`. Rejects with what is wrong in `world`.
    pub fn ui_start(
        &mut self,
        world: &str,
        kept: &str,
        map: &str,
        mode: &str,
        driving: bool,
    ) -> Result<(), JsValue> {
        // The game the scene was made for, with its trucks, is the one that is played.
        let mut shell = match self.game.take() {
            Some(shell) => shell,
            None => {
                let world = crate::shell::World::parse(world).map_err(|e| JsValue::from_str(&e))?;
                let mut shell = crate::shell::Shell::new(world, kept);
                shell.set_web();
                if !map.is_empty() {
                    shell.start_in(map);
                }
                if driving {
                    shell.resume(mode);
                }
                shell
            }
        };
        shell.enter(&self.gameplay);
        self.shell = Some(shell);
        Ok(())
    }

    /// What the game is doing, for the page to hand to the next page's [`WebGame`] when the
    /// game has asked for another map (`ui_orders`): the game is kept in its place first.
    pub fn ui_carry(&mut self) -> String {
        let Some(shell) = &mut self.shell else {
            return String::new();
        };
        let carried = shell.carry();
        if let Some(kept) = crate::shell::Shell::carried_kept(&carried) {
            keep_game(self.playing, kept);
        }
        carried
    }

    /// Does what was asked with the kept games: begins a new one in a place, goes back to
    /// one, or deletes one. What is being played is kept first.
    fn change_game(&mut self, what: crate::shell::Saves) {
        use crate::shell::Saves;
        let Some(shell) = &mut self.shell else {
            return;
        };
        if self.playing != 0 {
            keep_game(self.playing, &shell.keep_now());
        }
        let known = |slot: u32| (1..=SLOTS).contains(&slot);
        let kept = |slot: u32| {
            let mut games = kept_games().into_iter();
            games
                .find(|(number, _)| *number == slot)
                .and_then(|(_, kept)| kept)
        };
        let play = match what {
            Saves::Close => {
                self.playing = 0;
                shell.close();
                None
            }
            Saves::New(slot) if known(slot) => {
                keep_game(slot, "");
                Some(slot)
            }
            Saves::Use(slot) if known(slot) && kept(slot).is_some() => Some(slot),
            Saves::Delete(slot) if known(slot) => {
                if let Some(storage) = storage() {
                    let _ = storage.remove_item(&slot_key(slot));
                }
                if slot == self.playing {
                    self.playing = 0;
                }
                None
            }
            _ => None,
        };
        if let Some(slot) = play {
            self.playing = slot;
            shell.set_saves(&kept_games(), slot);
            shell.load(&kept(slot).unwrap_or_default());
        } else {
            shell.set_saves(&kept_games(), self.playing);
        }
    }

    /// Takes in a line the game's pages said (docs/ui.md).
    pub fn ui_say(&mut self, line: &str) {
        if let (Some(shell), Some(play)) = (&mut self.shell, &mut self.play) {
            shell.hear(line, &mut play.devices);
        }
    }

    /// What the game's pages should be told, as JSON, if there is anything since this was
    /// last asked; otherwise nothing.
    pub fn ui_told(&mut self) -> String {
        std::mem::take(&mut self.told)
    }

    /// What the game asks of the page that holds it, a line each: `quit`, or
    /// `load MAP SCENE MODE -`, or `load MAP SCENE MODE HOURS WEATHER SOAKED` when the map is
    /// to be driven at an hour, in a weather, after so many seconds of it.
    pub fn ui_orders(&mut self) -> Vec<String> {
        std::mem::take(&mut self.orders)
    }

    /// What the game wants kept until it is next played, if that has changed; otherwise
    /// nothing.
    pub fn ui_keep(&mut self) -> String {
        let kept = self.shell.as_mut().and_then(|shell| shell.to_keep());
        // It is kept here, in the place of the game that is being played: a free drive,
        // and the main menu, are kept nowhere.
        if let Some(kept) = &kept {
            keep_game(self.playing, kept);
        }
        String::new()
    }

    /// From now on what should be heard is kept for the page to take with `sound_asks`: call
    /// once the page can play it. Nothing happens if no game is running.
    pub fn sound_start(&mut self) {
        if let Some(play) = &mut self.play {
            play.with_web_sound();
        }
    }

    /// What the page is to play, as JSON: `loops`, each `[name, volume, pitch]`, which stand
    /// until asked for otherwise, and `shots`, the same, each to be played once, now. Nothing
    /// before `sound_start`.
    pub fn sound_asks(&mut self) -> String {
        self.play
            .as_mut()
            .map(|play| play.sound_asks())
            .unwrap_or_default()
    }

    /// Lets the weather there is have gone on for this many seconds already: a storm's
    /// ground is soaked from the start.
    pub fn soak(&mut self, seconds: f32) {
        self.renderer.advance_weather(seconds);
    }

    /// Whether a game is running with someone in it to play, who has the camera unless the
    /// player is flying.
    pub fn has_player(&self) -> bool {
        self.play
            .as_ref()
            .is_some_and(|play| play.play.has_player())
    }

    /// Whether the player has left whoever they play, to fly the camera.
    pub fn flying(&self) -> bool {
        self.play.as_ref().is_some_and(|play| play.play.flying())
    }

    /// Lets the player leave whoever they play and fly the camera, from where it is, or puts
    /// them back. Flying, the one left stands where they were; the rest of the game still
    /// hears the keys, so a door opens to the camera. Nothing changes in a game with no player.
    pub fn set_flying(&mut self, flying: bool) {
        if let Some(play) = &mut self.play {
            play.play.set_flying(flying);
        }
    }

    fn devices(&mut self) -> Option<&mut solstice_game::Devices> {
        self.play.as_mut().map(|play| &mut play.devices)
    }

    /// A key, by its `KeyboardEvent.code`, going down or up.
    pub fn key(&mut self, code: &str, down: bool) {
        if let Some(devices) = self.devices() {
            devices.key(code, down);
        }
    }

    /// A mouse button, `left`, `right` or `middle`, going down or up.
    pub fn mouse_button(&mut self, button: &str, down: bool) {
        if let Some(devices) = self.devices() {
            devices.mouse_button(button, down);
        }
    }

    /// The mouse moving by this many pixels, right and down.
    pub fn mouse_motion(&mut self, x: f32, y: f32) {
        if let Some(devices) = self.devices() {
            devices.mouse_motion(x, y);
        }
    }

    /// A gamepad button's value, from 0 to 1, by its name in the standard layout.
    pub fn gamepad_button(&mut self, button: &str, value: f32) {
        if let Some(devices) = self.devices() {
            devices.gamepad_button(button, value);
        }
    }

    /// A gamepad stick's axis, from -1 to 1: `left_x`, `left_y`, `right_x` or `right_y`.
    pub fn gamepad_axis(&mut self, axis: &str, value: f32) {
        if let Some(devices) = self.devices() {
            devices.gamepad_axis(axis, value);
        }
    }

    /// Where the touch stick is pushed, each axis from -1 to 1.
    pub fn touch_stick(&mut self, x: f32, y: f32) {
        if let Some(devices) = self.devices() {
            devices.touch_stick(x, y);
        }
    }

    /// A drag on the look side of the screen, by this many pixels.
    pub fn touch_look(&mut self, x: f32, y: f32) {
        if let Some(devices) = self.devices() {
            devices.touch_look(x, y);
        }
    }

    pub fn touch_tap(&mut self) {
        if let Some(devices) = self.devices() {
            devices.touch_tap();
        }
    }

    /// Lets every key and button go, for when the page loses focus.
    pub fn release_all(&mut self) {
        if let Some(devices) = self.devices() {
            devices.release_all();
        }
    }

    /// The game's scripts, by name.
    pub fn scripts(&self) -> Vec<String> {
        self.play
            .as_ref()
            .map_or_else(Vec::new, |play| play.play.scripts().to_vec())
    }

    /// Loads a new version of a script, keeping every actor's state. Rejects with the reason
    /// if it fails to load, and the old version keeps running.
    pub fn reload_script(&mut self, name: &str, source: &str) -> Result<(), JsValue> {
        let play = self
            .play
            .as_mut()
            .ok_or_else(|| JsValue::from_str("no game is running"))?;
        play.play
            .reload(name, source)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// The player's car's instruments, for the page to draw, or nothing if the player drives no
    /// car: speed in km/h, gear (-1 reverse, 0 neutral), revs a minute and where the limiter
    /// is, throttle and brake from 0 to 1, whether the anti-lock brakes, traction control and rev limiter are at work, laps
    /// finished, this lap's seconds, the last lap's and the best (each -1 if there is none),
    /// whether this lap still counts, and whether stability control is at work.
    pub fn hud(&self) -> Vec<f64> {
        let Some(hud) = self.play.as_ref().and_then(|play| play.play.hud()) else {
            return Vec::new();
        };
        let t = &hud.telemetry;
        let flag = |on: bool| f64::from(u8::from(on));
        let mut values = vec![
            f64::from(t.speed.abs() * 3.6),
            f64::from(t.gear),
            f64::from(t.rpm),
            f64::from(hud.limit),
            f64::from(hud.throttle),
            f64::from(hud.brake),
            flag(t.abs_active),
            flag(t.traction_active),
            flag(t.limiter),
        ];
        match &hud.lap {
            Some(lap) => values.extend([
                f64::from(lap.count),
                lap.current.unwrap_or(-1.0),
                lap.last.unwrap_or(-1.0),
                lap.best.unwrap_or(-1.0),
                flag(lap.valid),
            ]),
            None => values.extend([0.0, -1.0, -1.0, -1.0, 1.0]),
        }
        values.push(flag(t.stability_active));
        values
    }

    /// Whether the player is a car, so the page asks for [`WebViewer::hud`] only then.
    pub fn drives(&self) -> bool {
        self.play.as_ref().is_some_and(|play| play.play.drives())
    }

    /// The name of the circuit the player's car is on, or nothing.
    pub fn hud_track(&self) -> String {
        self.play
            .as_ref()
            .and_then(|play| play.play.hud())
            .map(|hud| hud.track.clone())
            .unwrap_or_default()
    }

    /// What scripts have logged, and every error, since this was last asked, oldest first.
    /// Errors start with `error: `.
    pub fn take_messages(&mut self) -> Vec<String> {
        self.messages
            .drain(..)
            .map(|m| {
                if m.error {
                    format!("error: {}", m.text)
                } else {
                    m.text
                }
            })
            .collect()
    }
}

/// Bytes to fetch first, to learn about the rest of a scene file.
#[wasm_bindgen]
pub fn header_bytes() -> u32 {
    file::HEADER_BYTES as u32
}

/// Reads the start of a scene file. Returns where the manifest starts and how large it is.
#[wasm_bindgen]
pub fn read_header(bytes: &[u8]) -> Result<Vec<f64>, JsValue> {
    let header = file::decode_header(bytes).map_err(error_string)?;
    Ok(vec![
        header.manifest_offset as f64,
        header.manifest_bytes as f64,
    ])
}

/// What the editor's page needs (`docs/editor.md`): the map of which entity made which
/// instances, moving and hiding instances, the camera's matrices, and picking.
#[cfg(feature = "editor")]
#[wasm_bindgen]
impl WebViewer {
    /// The ids of the entities the scene's map names, one to a line, in its order. Empty for a
    /// scene not baked from a project.
    pub fn source_ids(&self) -> String {
        let ids: Vec<&str> = self.sources.iter().map(|s| s.id.as_str()).collect();
        ids.join("\n")
    }

    /// Each mapped entity's instances, as its first and one past its last, in the map's order.
    pub fn source_ranges(&self) -> Vec<u32> {
        self.sources
            .iter()
            .flat_map(|s| [s.instances.start, s.instances.end])
            .collect()
    }

    /// Each mapped entity's placement as the bake made it, 16 numbers by column, in the map's
    /// order.
    pub fn source_placements(&self) -> Vec<f32> {
        self.sources
            .iter()
            .flat_map(|s| s.placement.to_cols_array())
            .collect()
    }

    pub fn instance_count(&self) -> u32 {
        self.renderer.instance_count()
    }

    /// Where an instance is drawn, 16 numbers by column; empty if there is no such instance.
    pub fn instance_transform(&self, instance: u32) -> Vec<f32> {
        self.renderer
            .instance_transform(instance)
            .map(|m| m.to_cols_array().to_vec())
            .unwrap_or_default()
    }

    /// An instance's bounding sphere in the world: centre x, y, z and radius; empty if none.
    pub fn instance_bounds(&self, instance: u32) -> Vec<f32> {
        self.renderer
            .instance_bounds(instance)
            .map(|(c, r)| vec![c.x, c.y, c.z, r])
            .unwrap_or_default()
    }

    /// Puts an instance at `world` (16 numbers by column) from the next frame. `jump` puts it
    /// there with no motion between, as an undo should; otherwise it moves there, as a drag does.
    /// False if there is no such instance or the matrix can't be inverted.
    pub fn set_instance_transform(&mut self, instance: u32, world: &[f32], jump: bool) -> bool {
        let Ok(columns) = <[f32; 16]>::try_from(world) else {
            return false;
        };
        let world = glam::Mat4::from_cols_array(&columns);
        if !world.is_finite() || world.determinant().abs() < 1e-12 {
            return false;
        }
        if jump {
            self.renderer.place_instance(instance, world)
        } else {
            self.renderer.move_instance(instance, world)
        }
    }

    /// Makes an instance movable while it is dragged, and static again after, so the cached
    /// shadows and GI are redrawn once at the end rather than every frame of the drag.
    pub fn set_instance_movable(&mut self, instance: u32, movable: bool) -> bool {
        self.renderer.set_instance_movable(instance, movable)
    }

    pub fn set_instance_hidden(&mut self, instance: u32, hidden: bool) -> bool {
        self.renderer.set_instance_hidden(instance, hidden)
    }

    /// The camera's view and projection, 16 numbers by column each, for the page to draw the
    /// gizmo over the scene and turn the mouse into rays. The projection has reversed depth and
    /// no far plane.
    pub fn view_projection(&self) -> Vec<f32> {
        let camera = &self.camera.camera;
        let aspect = self.config.width as f32 / self.config.height.max(1) as f32;
        let mut out = camera.view().to_cols_array().to_vec();
        out.extend(camera.projection(aspect).to_cols_array());
        out
    }

    /// How fast the camera flies, in metres a second (Shift goes five times faster).
    pub fn set_camera_speed(&mut self, metres_per_second: f32) {
        if metres_per_second.is_finite() && metres_per_second > 0.0 {
            self.camera.speed = metres_per_second;
        }
    }

    pub fn camera_speed(&self) -> f32 {
        self.camera.speed
    }

    /// Asks what is under a point of the canvas, as fractions of its width and height from the
    /// top left. `take_pick` gives the answer a frame or two later.
    pub fn request_pick(&mut self, x: f32, y: f32) {
        self.renderer.request_pick(x, y);
    }

    /// The last pick's answer, once: the instance (or -1 for the sky) and its reversed depth.
    /// Empty while it isn't ready.
    pub fn take_pick(&mut self) -> Vec<f64> {
        match self.renderer.take_pick() {
            None => Vec::new(),
            Some(picked) => vec![
                picked.instance.map_or(-1.0, f64::from),
                f64::from(picked.depth),
            ],
        }
    }
}
