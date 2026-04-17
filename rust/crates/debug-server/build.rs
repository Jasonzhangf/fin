use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=webui/src");
    println!("cargo:rerun-if-changed=webui/index.html");
    println!("cargo:rerun-if-changed=webui/styles.css");
    println!("cargo:rerun-if-changed=webui/tsconfig.json");

    let status = Command::new("npx")
        .args(["tsc", "-p", "webui/tsconfig.json"])
        .status()
        .expect("failed to execute npx tsc for debug-server webui");

    if !status.success() {
        panic!("webui TypeScript compilation failed: {status}");
    }
}