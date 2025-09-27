use regex::Regex;
use std::os::windows::process::CommandExt;

static DEBUG: bool = cfg!(debug_assertions);
static COMPILED_TIME: &str = env!("VERGEN_BUILD_TIMESTAMP");
static VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let current_time = chrono::Local::now();
    println!("EXE VERSION:\t{}", VERSION);
    println!(
        "COMPILED AT:\t{} {}",
        &COMPILED_TIME[0..10],
        &COMPILED_TIME[11..19]
    );
    println!("RUN AT:\t\t{}", current_time.format("%Y-%m-%d %H:%M:%S"));

    print_formatted(get_format_info(get_raw_info()));

    if !DEBUG {
        pause();
    }
}

fn get_format_info(raws: String) -> Vec<(String, String)> {
    let mut lines = raws.lines();
    let headers = lines.next().unwrap_or_default();
    let values = lines.next().unwrap_or_default();
    let mut raw_pairs: Vec<(String, String)> = headers
        .split(",\"")
        .zip(values.split(",\""))
        .map(|(key, value)| {
            (
                key.trim_matches('"').to_owned(),
                value.trim_matches('"').to_owned(),
            )
        })
        .collect();

    raw_pairs.push(("GPU".to_string(), {
        let ram = get_wmic_info("path win32_VideoController", "AdapterRAM")
            .parse::<u64>()
            .unwrap_or(0);
        let name = get_wmic_info("path win32_VideoController", "NAME");
        let bits = get_wmic_info("path win32_VideoController", "CurrentBitsPerPixel");
        let h = get_wmic_info("path win32_VideoController", "CurrentHorizontalResolution");
        let v = get_wmic_info("path win32_VideoController", "CurrentVerticalResolution");
        format!(
            "Name:\t\t\t{}\n\
            AdapterRAM:\t\t{} GB\n\
            CurrentBitsPerPixel:\t{}\n\
            CurrentResolution:\t{}x{}",
            name,
            (ram as f64 / 1024.0 / 1024.0 / 1024.0).round(),
            bits,
            h,
            v
        )
    }));

    raw_pairs.push((
        "SerialNumber".to_string(),
        get_wmic_info("BIOS", "SerialNumber"),
    ));

    raw_pairs.push(("CPU".to_string(), {
        let name = get_wmic_info("CPU", "Name");
        let cores = get_wmic_info("CPU", "NumberOfCores");
        let clock = get_wmic_info("CPU", "MaxClockSpeed");

        format!(
            "Name:\t\t{}\nNumberOfCores:\t{}\nMaxClockSpeed:\t{}",
            name, cores, clock
        )
    }));
    raw_pairs.push(("Disks".to_string(), {
        let output = std::process::Command::new("cmd")
            .args(["/C","chcp 65001 > nul && wmic diskdrive get InterfaceType,MediaType,Model,SerialNumber,Size /format:csv"])
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        let raw_str = String::from_utf8(output.stdout).unwrap();
        let rows: Vec<Vec<String>> = raw_str
            .trim().replace("\r\n", "\n").lines()
            .map(|line| {
                line.split(',')
                    .map(String::from)
                    .collect()
            })
            .collect();

        let mut sum: Vec<String> = Vec::new();
        for (i,j) in rows.iter().enumerate(){
            if i ==0{continue;}
            let size = j[5].parse::<u64>().unwrap_or(0);
            let each_disk = format!(
                "[Disk {}]\n\
                 Model:\t\t{}\n\
                 SerialNumber:\t{}\n\
                 Size:\t\t{} GB\n\
                 MediaType:\t{}\n\
                 InterfaceType:\t{}",
                i,
                &j[3],
                &j[4],
                (size as f64 /1024.0/1024.0/1024.0).round(),
                &j[2],
                &j[1],
            );
            sum.push(each_disk);
        }

        sum.join("\n")
    }));

    raw_pairs
}

fn get_raw_info() -> String {
    let output = std::process::Command::new("cmd")
        .args(["/C", "chcp 65001 > nul && systeminfo /FO CSV"])
        .creation_flags(0x08000000)
        .output()
        .unwrap();

    let raw_str = String::from_utf8_lossy(&output.stdout);

    raw_str.trim().to_string()
}

fn print_formatted(infos: Vec<(String, String)>) {
    //dbg!(&infos);
    let mut formatted_map: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();

    let re_temp_version = Regex::new(r"(\d+\.\d+\.\d+).*").unwrap();
    let re_temp_arch = Regex::new(r"(.*?)\-.*").unwrap();
    let re_temp_memory = Regex::new(r"^([\d,]+)\s(\w+)$").unwrap();
    let re_temp_net_card = Regex::new(r"^\[01\]:\s(.*)$").unwrap();
    let re_temp_ip = Regex::new(r"([\d\.]+)").unwrap();

    for (i, j) in infos {
        match i.as_str() {
            "OS Name" => {
                formatted_map.insert("OS".to_string(), j);
            }
            "OS Version" => {
                let detailed_version = cap_group_trim(&re_temp_version.captures(&j).unwrap(), 1);

                if let Some(x) = formatted_map.get_mut("OS") {
                    x.push_str(&format!(" ({})", detailed_version));
                }
            }
            "System Type" => {
                let arch = cap_group_trim(&re_temp_arch.captures(&j).unwrap(), 1);

                if let Some(x) = formatted_map.get_mut("OS") {
                    x.push_str(&format!(" {}", arch));
                }
            }
            "System Manufacturer" => {
                formatted_map.insert(i, j);
            }
            "BIOS Version" => {
                formatted_map.insert("BIOS".to_string(), format!("Version:\t{}", j));
            }
            "System Locale" => {
                formatted_map.insert("Language".to_string(), j);
            }
            "Time Zone" => {
                formatted_map.insert(i, j);
            }
            "Total Physical Memory" => {
                let temp = re_temp_memory.captures(&j[..]).unwrap();

                let num_str = cap_group_trim(&temp, 1);
                let size_str = cap_group_trim(&temp, 2).to_lowercase();

                let num = num_str.replace(",", "").parse::<f64>().unwrap_or(0.0);
                let num_to_gb: f64 = match size_str.as_str() {
                    "tb" => 1000.0,
                    "gb" => 1.0,
                    "mb" => 0.001,
                    "kb" => 0.000001,
                    _ => 0.0,
                };

                formatted_map.insert(
                    "Memory".to_string(),
                    format!("Physical:\t{} GB", (num * num_to_gb).round()),
                );
            }
            "Virtual Memory: Max Size" => {
                let temp = re_temp_memory.captures(&j[..]).unwrap();

                let num_str = cap_group_trim(&temp, 1);
                let size_str = cap_group_trim(&temp, 2).to_lowercase();

                let num = num_str.replace(",", "").parse::<f64>().unwrap_or(0.0);
                let num_to_gb: f64 = match size_str.as_str() {
                    "tb" => 1000.0,
                    "gb" => 1.0,
                    "mb" => 0.001,
                    "kb" => 0.000001,
                    _ => 0.0,
                };

                if let Some(x) = formatted_map.get_mut("Memory") {
                    x.push_str(&format!("\nVirtual:\t{} GB", (num * num_to_gb).round()));
                };
            }
            "Network Card(s)" => {
                let temp = j.split(",").collect::<Vec<&str>>();
                for item in temp {
                    if re_temp_net_card.is_match(item) {
                        formatted_map.insert(
                            "Network".to_string(),
                            format!("Card:\t\t{}", item.replace("[01]: ", "")),
                        );
                    }
                    if item.contains("DHCP Server")
                        && let Some(x) = formatted_map.get_mut("Network")
                    {
                        x.push_str(&format!(
                            "\nDHCP Server:\t{}",
                            cap_group_trim(&re_temp_ip.captures(item).unwrap(), 1)
                        ));
                    }
                }
            }
            "Virtualization-based security" => {
                if j.contains("Secure Boot")
                    && let Some(x) = formatted_map.get_mut("BIOS")
                {
                    x.push_str("\nSecure Boot:\tEnabled");
                }
            }
            "SerialNumber" => {
                if let Some(x) = formatted_map.get_mut("BIOS") {
                    x.push_str(&format!("\nSerialNumber:\t{}", j));
                }
            }
            "CPU" => {
                formatted_map.insert(i, j);
            }
            "GPU" => {
                formatted_map.insert(i, j);
            }
            "Disks" => {
                formatted_map.insert(i, j);
            }
            _ => {}
        }
    }

    let sort = [
        "OS",
        "System Manufacturer",
        "BIOS",
        "Language",
        "Time Zone",
        "CPU",
        "Memory",
        "GPU",
        "Disks",
        "Network",
    ];
    for i in sort {
        print!("\n------------\n");
        println!("{}:", i);
        println!("{}", formatted_map.get(i).unwrap_or(&String::new()));
        //print!("\n------------\n");
    }
}

fn get_wmic_info(which: impl ToString, what: impl ToString) -> String {
    let mut args: Vec<String> = Vec::new();
    args.push("/C".to_string());
    args.push(format!(
        "chcp 65001 > nul && wmic {} get {}",
        which.to_string().trim(),
        what.to_string().trim()
    ));

    let output = std::process::Command::new("cmd")
        .args(args)
        .creation_flags(0x08000000)
        .output()
        .unwrap();

    let raw_str = String::from_utf8_lossy(&output.stdout);

    raw_str
        .into_owned()
        .lines()
        .nth(1)
        .unwrap()
        .trim()
        .to_string()
}

fn pause() -> ! {
    println!("\nPress Enter to exit...");
    let mut temp = String::new();
    let _ = std::io::stdin().read_line(&mut temp);
    std::process::exit(0);
}

fn cap_group_trim(caps: &regex::Captures, idx: usize) -> String {
    caps.get(idx)
        .map(|m| m.as_str().trim().to_owned())
        .unwrap_or_default()
}
