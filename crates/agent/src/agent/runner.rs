use std::collections::HashMap;

use std::io::BufRead;
use std::process::Command;
use std::sync::mpsc;
use std::thread::spawn;

pub struct Runner {
    processes: HashMap<u32, mpsc::Sender<()>>,
}

impl Runner {
    pub fn new() -> Runner {
        Runner {
            processes: HashMap::new(),
        }
    }

    pub fn start(&mut self, name: &str) -> u32 {
        let id = self.processes.len() as u32;
        
        println!("Starting process {} with name {}", id, name);

        let (tx, rx) = mpsc::channel();
        self.processes.insert(id, tx);

        let name_clone = name.to_owned(); // Clone the name string

        let _process = spawn(move || {
            let mut child = Command::new(name_clone)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap();

            let stdout = child.stdout.take().unwrap();
            let reader = std::io::BufReader::new(stdout);

            for line in reader.lines() {
                if let Ok(line) = line {
                    println!("Process {}: {}", id, line);
                }
            }

            loop {
                match rx.try_recv() {
                    Ok(_) => {
                        println!("Killing process {}", id);
                        child.kill().unwrap();
                        break;
                    }
                    Err(_) => {}
                }
            }
        });

        id
    }

    pub fn kill(&mut self, id: u32) {
        match self.processes.remove(&id) {
            Some(tx) => {
                tx.send(()).unwrap();
            }
            None => {}
        }
    }
}