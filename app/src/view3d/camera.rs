use glam::camera::rh::{proj::directx as proj, view};
use glam::{Mat4, Vec3};

/// Kamera orbitalna, oś Z do góry.
pub struct OrbitCamera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
    pub orthographic: bool,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        let mut c = Self {
            target: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            distance: 300.0,
            fov_y: 35f32.to_radians(),
            orthographic: false,
        };
        c.set_iso();
        c
    }
}

impl OrbitCamera {
    pub fn set_iso(&mut self) {
        self.yaw = (-60f32).to_radians();
        self.pitch = 35f32.to_radians();
    }
    pub fn set_top(&mut self) {
        self.yaw = (-90f32).to_radians();
        self.pitch = 89.9f32.to_radians();
    }
    pub fn set_bottom(&mut self) {
        self.yaw = (-60f32).to_radians();
        self.pitch = (-35f32).to_radians();
    }
    pub fn set_front(&mut self) {
        self.yaw = (-90f32).to_radians();
        self.pitch = 0.0;
    }

    pub fn forward(&self) -> Vec3 {
        -Vec3::new(self.pitch.cos() * self.yaw.cos(), self.pitch.cos() * self.yaw.sin(), self.pitch.sin())
    }

    pub fn eye(&self) -> Vec3 {
        self.target - self.forward() * self.distance
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Z).normalize_or(Vec3::X)
    }

    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward())
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.008;
        self.pitch = (self.pitch + dy * 0.008).clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
    }

    /// Przesunięcie o piksele ekranu przy wysokości widoku `height_px`.
    pub fn pan(&mut self, dx: f32, dy: f32, height_px: f32) {
        let world_per_px = 2.0 * self.distance * (self.fov_y * 0.5).tan() / height_px.max(1.0);
        self.target += (-self.right() * dx + self.up() * dy) * world_per_px;
    }

    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(0.5, 1e6);
    }

    /// Dopasowanie do sfery (środek, promień) przy proporcjach widoku `aspect`.
    pub fn fit(&mut self, center: Vec3, radius: f32, aspect: f32) {
        self.target = center;
        let fov_x = 2.0 * ((self.fov_y * 0.5).tan() * aspect).atan();
        let fov = self.fov_y.min(fov_x);
        self.distance = radius.max(1.0) / (fov * 0.5).sin() * 1.05;
    }

    pub fn view_proj(&self, aspect: f32, scene_radius: f32) -> Mat4 {
        let view = view::look_at_mat4(self.eye(), self.target, Vec3::Z);
        let far = self.distance + scene_radius * 4.0 + 1000.0;
        let proj = if self.orthographic {
            let h = self.distance * (self.fov_y * 0.5).tan();
            proj::orthographic(-h * aspect, h * aspect, -h, h, -far, far)
        } else {
            let near = (self.distance * 0.01).max(0.05);
            proj::perspective(self.fov_y, aspect, near, far)
        };
        proj * view
    }
}
