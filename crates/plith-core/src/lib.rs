//! Pure logic with no Windows dependency: springs, the vsync timeline, motion statistics, the island state machine, geometry and page layout.

pub mod clock_choice;
pub mod demo;
pub mod dwm;
pub mod geometry;
pub mod gpu_health;
pub mod island;
pub mod launch;
pub mod layout;
pub mod levers;
pub mod motion;
pub mod page;
pub mod scene;
pub mod shapes;
pub mod spring;
pub mod timeline;
pub mod topmost;
pub mod tuning;
pub mod visible;
