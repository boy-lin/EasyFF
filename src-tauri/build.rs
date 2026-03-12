fn main() {
    // 调用 tauri_build::build() - 这会触发所有依赖的 build script
    // 此时环境变量已经设置，子进程应该能看到
    tauri_build::build()
}
