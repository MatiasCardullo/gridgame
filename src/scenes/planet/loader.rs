use super::*;

impl PlanetLoader {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut log_file = File::create("loading.log").ok().map(BufWriter::new);
            let total_steps: u32 = 5;
            let mut done_steps: u32 = 0;

            fn send_log(
                tx: &mpsc::Sender<PlanetLoadEvent>,
                log_file: &mut Option<BufWriter<File>>,
                line: String,
            ) {
                if let Some(file) = log_file.as_mut() {
                    let _ = writeln!(file, "{line}");
                    let _ = file.flush();
                }
                let _ = tx.send(PlanetLoadEvent::LogLine(line));
            }

            fn step_start(
                tx: &mpsc::Sender<PlanetLoadEvent>,
                log_file: &mut Option<BufWriter<File>>,
                name: &str,
            ) -> Instant {
                let line = format!(
                    "{} Step start: {}",
                    format_log_timestamp(SystemTime::now()),
                    name
                );
                send_log(tx, log_file, line);
                let _ = tx.send(PlanetLoadEvent::StepStart(name.to_string()));
                Instant::now()
            }

            fn step_done(
                tx: &mpsc::Sender<PlanetLoadEvent>,
                log_file: &mut Option<BufWriter<File>>,
                name: &str,
                start: Instant,
                done_steps: &mut u32,
                total_steps: u32,
            ) {
                let elapsed_ms = start.elapsed().as_millis();
                let line = format!(
                    "{} Step done: {} ({} ms)",
                    format_log_timestamp(SystemTime::now()),
                    name,
                    elapsed_ms
                );
                send_log(tx, log_file, line);
                let _ = tx.send(PlanetLoadEvent::StepDone);
                *done_steps += 1;
                let _ = tx.send(PlanetLoadEvent::Progress {
                    done: *done_steps,
                    total: total_steps,
                });
            }

            let mut config = PlanetNoiseConfig::default();
            let mut loaded_sector_values: Option<Vec<f32>> = None;
            if let Ok(contents) = fs::read_to_string(PLANET_NOISE_PATH) {
                if let Ok(snapshot) = serde_json::from_str::<PlanetNoiseSnapshot>(&contents) {
                    config = snapshot.config;
                    loaded_sector_values = Some(snapshot.sector_values);
                }
            }

            let geometry_start = step_start(&tx, &mut log_file, "Build geometry");
            let base_vertices = align_vertices_to_poles(&icosahedron_vertices());
            let base_faces = build_faces(&base_vertices, &icosahedron_edges());
            let (sector_vertices, sector_faces) =
                build_geodesic_sphere(&base_vertices, &base_faces, 4, 10, 1.0);
            step_done(
                &tx,
                &mut log_file,
                "Build geometry",
                geometry_start,
                &mut done_steps,
                total_steps,
            );

            let heightmap_start = step_start(&tx, &mut log_file, "Load/build heightmap");
            ensure_planet_data_dir();
            let heightmap_pixels = load_or_build_heightmap(&config, HEIGHTMAP_SIZE);
            step_done(
                &tx,
                &mut log_file,
                "Load/build heightmap",
                heightmap_start,
                &mut done_steps,
                total_steps,
            );

            let texture_mesh_start = step_start(&tx, &mut log_file, "Build texture mesh data");
            let (base_texture_mesh_data, _, _) = build_planet_chunk_data(
                &sector_vertices,
                &sector_faces,
                TEXTURE_BASE_SUBDIVISIONS,
                1.6 + TEXTURE_LAYER_OFFSET,
                &config,
                false,
                true,
            );
            step_done(
                &tx,
                &mut log_file,
                "Build texture mesh data",
                texture_mesh_start,
                &mut done_steps,
                total_steps,
            );

            let texture_data = PlanetTextureData {
                config,
                base_vertices: base_vertices.clone(),
                sector_vertices: sector_vertices.clone(),
                sector_faces: sector_faces.clone(),
                base_texture_mesh_data: base_texture_mesh_data.clone(),
                heightmap_pixels: heightmap_pixels.clone(),
            };
            let _ = tx.send(PlanetLoadEvent::TextureReady(texture_data));

            let relief_start = step_start(&tx, &mut log_file, "Build relief mesh data");
            let (mesh_data, face_normals, sector_values_raw) = build_planet_chunk_data(
                &sector_vertices,
                &sector_faces,
                config.subdivisions as usize,
                1.6,
                &config,
                true,
                false,
            );
            let sector_values = loaded_sector_values.unwrap_or(sector_values_raw);
            step_done(
                &tx,
                &mut log_file,
                "Build relief mesh data",
                relief_start,
                &mut done_steps,
                total_steps,
            );

            let save_start = step_start(&tx, &mut log_file, "Save mesh points");
            save_mesh_points(&mesh_data);
            step_done(
                &tx,
                &mut log_file,
                "Save mesh points",
                save_start,
                &mut done_steps,
                total_steps,
            );

            let data = PlanetBuildData {
                config,
                base_vertices,
                sector_vertices,
                sector_faces,
                mesh_data,
                base_texture_mesh_data,
                face_normals,
                sector_values,
                heightmap_pixels,
            };

            let _ = tx.send(PlanetLoadEvent::Done(data));
        });

        Self {
            status: PlanetLoadStatus::Loading,
            progress: 0.0,
            current_step: None,
            log_entries: Vec::new(),
            rx: Some(rx),
            pending_texture_data: None,
            pending_data: None,
            error: None,
        }
    }

    pub fn poll(&mut self) -> Option<PlanetBuildData> {
        let Some(rx) = self.rx.as_ref() else {
            return self.pending_data.take();
        };
        let mut pending_logs: Vec<String> = Vec::new();
        while let Ok(message) = rx.try_recv() {
            match message {
                PlanetLoadEvent::StepStart(name) => {
                    self.current_step = Some(name);
                }
                PlanetLoadEvent::StepDone => {}
                PlanetLoadEvent::Progress { done, total } => {
                    let total = total.max(1);
                    self.progress = (done as f32 / total as f32).clamp(0.0, 1.0);
                }
                PlanetLoadEvent::LogLine(line) => {
                    pending_logs.push(line);
                }
                PlanetLoadEvent::TextureReady(data) => {
                    self.pending_texture_data = Some(data);
                }
                PlanetLoadEvent::Done(data) => {
                    self.status = PlanetLoadStatus::Done;
                    self.progress = 1.0;
                    self.pending_data = Some(data);
                }
            }
        }
        for line in pending_logs {
            self.push_log(line);
        }
        self.pending_data.take()
    }

    pub fn take_texture_data(&mut self) -> Option<PlanetTextureData> {
        self.pending_texture_data.take()
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.status, PlanetLoadStatus::Done)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.status, PlanetLoadStatus::Loading)
    }

    pub fn progress(&self) -> f32 {
        self.progress
    }

    pub fn log_entries(&self) -> &[String] {
        self.log_entries.as_slice()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn push_log(&mut self, line: String) {
        const MAX_LINES: usize = 12;
        self.log_entries.push(line);
        if self.log_entries.len() > MAX_LINES {
            let excess = self.log_entries.len() - MAX_LINES;
            self.log_entries.drain(0..excess);
        }
    }
}
