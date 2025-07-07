use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Clean up any lingering processes using sentiment ports
    let sentiment_ports = [3001, 4001, 5001, 6001, 80]; // Add all relevant ports
    for port in &sentiment_ports {
        println!("Attempting to kill processes on port {}", port);
        let output = Command::new("lsof").arg(format!("-i:{}", port)).output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("LISTEN") || line.contains("ESTABLISHED") {
                    if let Some(pid_str) = line.split_whitespace().nth(1) {
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            println!("Killing process {} on port {}", pid, port);
                            Command::new("kill")
                                .arg("-9")
                                .arg(pid.to_string())
                                .output()
                                .ok(); // Ignore kill errors
                        }
                    }
                }
            }
        }
    }

    tonic_build::compile_protos("proto/market_gateway.proto")?;
    Ok(())
}
