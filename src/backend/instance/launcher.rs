use crate::backend::auth::microsoft::Account;
use crate::backend::core::mojang::{
    is_rule_allowed, ComponentMeta, MavenCoordinate,
};
use crate::backend::instance::manager::{Instance, MmcPack};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub enum LaunchMsg {
    Log(String),
    Error(String),
    Finished(i32),
}
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub java_path: PathBuf,
    pub shared_data_path: PathBuf,
    pub mc_data_path: PathBuf,
    pub account: Option<Account>,
    pub jvm_args: Vec<String>,
    pub max_memory: u32, // In MB
    pub min_memory: u32, // In MB
    pub is_java_automatic: bool,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        let mc_data = crate::config::Config::get_data_dir();

        Self {
            java_path: PathBuf::from("java"),
            shared_data_path: mc_data.clone(),
            mc_data_path: mc_data,
            account: None,
            jvm_args: Vec::new(),
            max_memory: 4096,
            min_memory: 512,
            is_java_automatic: true,
        }
    }
}

/// Extract native .so/.dll files from a classifier JAR into the given directory.
fn extract_natives_jar(jar_path: &Path, natives_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(jar_path)
        .map_err(|e| format!("Failed to open natives jar {:?}: {}", jar_path, e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read natives jar {:?}: {}", jar_path, e))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();

        // Skip directories and META-INF
        if entry.is_dir() || name.starts_with("META-INF") {
            continue;
        }

        // Only extract native shared libraries
        if name.ends_with(".so") || name.ends_with(".dll") || name.ends_with(".dylib") {
            // Use just the filename, strip any subdirectory path
            let file_name = Path::new(&name)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(name.clone());

            let out_path = natives_dir.join(&file_name);
            let mut out_file = fs::File::create(&out_path)
                .map_err(|e| format!("Failed to create native file {:?}: {}", out_path, e))?;
            std::io::copy(&mut entry, &mut out_file)
                .map_err(|e| format!("Failed to extract native {:?}: {}", name, e))?;
        }
    }
    Ok(())
}

fn resolve_library_path(lib_name: &str, data_path: &Path) -> PathBuf {
    MavenCoordinate::parse(lib_name)
        .map(|c| c.to_full_path(&data_path.join("libraries")))
        .unwrap_or_default()
}

fn is_modern_minecraft_version(mc_version: &str) -> bool {
    let parts: Vec<&str> = mc_version.split('.').collect();
    if !parts.is_empty() {
        let first = parts[0];
        if first == "1" {
            if parts.len() >= 2 {
                if let Ok(minor) = parts[1].parse::<u32>() {
                    if minor > 20 {
                        return true;
                    }
                    if minor == 20 && parts.len() >= 3 {
                        let patch_str = parts[2];
                        let clean_patch: String = patch_str.chars().take_while(|c| c.is_ascii_digit()).collect();
                        if let Ok(patch) = clean_patch.parse::<u32>() {
                            return patch >= 6;
                        }
                    }
                }
            }
        } else {
            // Snapshot or year-based versions (e.g. 26.1)
            let leading_digits: String = first.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(year_ver) = leading_digits.parse::<u32>() {
                if year_ver >= 24 {
                    return true;
                }
            }
        }
    }
    false
}

#[derive(Default)]
struct OrderedClasspath {
    keys: Vec<String>,
    paths: Vec<PathBuf>,
}

impl OrderedClasspath {
    fn insert(&mut self, key: String, path: PathBuf) {
        if let Some(pos) = self.keys.iter().position(|k| k == &key) {
            self.paths[pos] = path;
        } else {
            self.keys.push(key);
            self.paths.push(path);
        }
    }

    fn into_paths(self) -> Vec<PathBuf> {
        self.paths
    }
}

fn parse_args_string(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for c in input.chars() {
        match c {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = c;
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
            }
            ' ' | '\t' | '\r' | '\n' if !in_quotes => {
                if !current.is_empty() {
                    args.push(current);
                    current = String::new();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn substitute_vars(token: &str, vars: &[(&str, &str)]) -> String {
    let mut s = token.to_string();
    for (k, v) in vars {
        s = s.replace(k, v);
    }
    s
}

pub fn launch_instance(
    instance: &Instance,
    options: LaunchOptions,
) -> Result<std::process::Child, String> {
    let pack_path = instance.path.join("mmc-pack.json");
    let pack_content = fs::read_to_string(&pack_path)
        .map_err(|e| format!("Failed to read mmc-pack.json: {}", e))?;
    let pack: MmcPack = serde_json::from_str(&pack_content)
        .map_err(|e| format!("Failed to parse mmc-pack.json: {}", e))?;

    let mut classpath_map = OrderedClasspath::default();
    let mut main_class = String::new();
    let mut game_arg_tokens: Vec<String> = Vec::new();
    let mut jvm_args_template: Vec<String> = Vec::new();
    let mut asset_index_id = String::from("legacy");
    let mut is_forge = false;
    let mut forge_version: Option<String> = None;
    let mut _is_neoforge = false;
    let mut _neoforge_version: Option<String> = None;
    let mut native_jar_paths: Vec<PathBuf> = Vec::new();
    let mut extra_tweakers: Vec<String> = Vec::new();

    for component in &pack.components {
        let is_loader = component.uid.contains("loader")
            || component.uid.contains("forge")
            || component.uid.contains("quilt")
            || component.uid.contains("neoforged");

        // Track Forge component
        if component.uid == "net.minecraftforge" {
            is_forge = true;
            forge_version = Some(component.version.clone());
        }
        // Track NeoForge component
        if component.uid == "net.neoforged" {
            _is_neoforge = true;
            _neoforge_version = Some(component.version.clone());
        }

        let meta_path = options
            .mc_data_path
            .join("meta")
            .join(&component.uid)
            .join(format!("{}.json", component.version));

        let meta_path = if meta_path.exists() {
            meta_path
        } else {
            options
                .shared_data_path
                .join("meta")
                .join(&component.uid)
                .join(format!("{}.json", component.version))
        };

        if let Ok(meta_content) = fs::read_to_string(meta_path) {
            if let Ok(meta) = serde_json::from_str::<ComponentMeta>(&meta_content) {
                if let Some(cls) = meta.main_class {
                    if main_class.is_empty() || is_loader {
                        main_class = cls;
                    }
                }
                // Collect +tweakers from Prism Meta (e.g. FMLTweaker for Forge)
                for tweaker in &meta.tweakers {
                    if !extra_tweakers.contains(tweaker) {
                        extra_tweakers.push(tweaker.clone());
                    }
                }
                if let Some(ref args) = meta.arguments {
                    if let Some(ref game_args) = args.game {
                        for arg in game_args {
                            if let Some(s) = arg.as_str() {
                                game_arg_tokens.push(s.to_string());
                            }
                        }
                    }
                } else if let Some(args) = meta.minecraft_arguments {
                    let parsed = parse_args_string(&args);
                    if !parsed.is_empty() {
                        game_arg_tokens = parsed;
                    }
                }
                // Collect JVM args from modern arguments.jvm
                if let Some(ref args) = meta.arguments {
                    if let Some(ref jvm_args) = args.jvm {
                        for arg in jvm_args {
                            if let Some(s) = arg.as_str() {
                                // Skip -cp and ${classpath} since we handle those ourselves
                                if s != "-cp" && s != "${classpath}" {
                                    jvm_args_template.push(s.to_string());
                                }
                            }
                            // Conditional JVM args (objects with rules + value)
                            else if let Some(obj) = arg.as_object() {
                                // Check if rules allow this arg on linux
                                let mut allowed = true;
                                if let Some(rules) = obj.get("rules") {
                                    if let Some(rules_arr) = rules.as_array() {
                                        allowed = false;
                                        for rule in rules_arr {
                                            let action = rule
                                                .get("action")
                                                .and_then(|a| a.as_str())
                                                .unwrap_or("");
                                            let os_name = rule
                                                .get("os")
                                                .and_then(|o| o.get("name"))
                                                .and_then(|n| n.as_str());
                                            if action == "allow" {
                                                if let Some(name) = os_name {
                                                    if name == "linux" {
                                                        allowed = true;
                                                    }
                                                } else {
                                                    allowed = true;
                                                }
                                            } else if action == "disallow" {
                                                if let Some(name) = os_name {
                                                    if name == "linux" {
                                                        allowed = false;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if allowed {
                                    if let Some(value) = obj.get("value") {
                                        if let Some(s) = value.as_str() {
                                            if s != "-cp" && s != "${classpath}" {
                                                jvm_args_template.push(s.to_string());
                                            }
                                        } else if let Some(arr) = value.as_array() {
                                            for v in arr {
                                                if let Some(s) = v.as_str() {
                                                    if s != "-cp" && s != "${classpath}" {
                                                        jvm_args_template.push(s.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(idx) = meta.asset_index {
                    asset_index_id = idx.id;
                }
                if let Some(libs) = meta.libraries {
                    for lib in libs {
                        if is_rule_allowed(&lib.rules) {
                            // Collect the main artifact for classpath
                            let mut path = resolve_library_path(&lib.name, &options.mc_data_path);
                            if !path.exists() {
                                path = resolve_library_path(&lib.name, &options.shared_data_path);
                            }
                            if path.exists() {
                                let parts: Vec<&str> = lib.name.split(':').collect();
                                if parts.len() >= 2 {
                                    let mut artifact = parts[1].to_string();
                                    let mut classifier = if parts.len() > 3 {
                                        Some(
                                            parts[3]
                                                .split('@')
                                                .next()
                                                .unwrap_or(parts[3])
                                                .to_string(),
                                        )
                                    } else {
                                        None
                                    };

                                    // Normalize LWJGL natives-as-artifact-name to natives-as-classifier
                                    if artifact.starts_with("lwjgl")
                                        && artifact.contains("-natives-")
                                    {
                                        if let Some(pos) = artifact.find("-natives-") {
                                            let actual_classifier = artifact[pos + 1..].to_string();
                                            artifact = artifact[..pos].to_string();
                                            classifier = Some(actual_classifier);
                                        }
                                    }

                                    let mut key = format!("{}:{}", parts[0], artifact);
                                    if let Some(c) = classifier {
                                        key.push_str(&format!(":{}", c));
                                    }
                                    classpath_map.insert(key, path);
                                }
                            }

                            // LWJGL 3+: natives are shipped as separate classifier JARs
                            // (e.g. org.lwjgl:lwjgl:3.4.1:natives-linux) that must be
                            // ON the classpath so the JVM can load them via its built-in
                            // JAR extraction. Do NOT rely on -Djava.library.path for these.
                            // Only attempt to find/append natives-linux if the library name doesn't already have a classifier.
                            let parts: Vec<&str> = lib.name.split(':').collect();
                            if parts.len() == 3 {
                                let native_classifier_name = format!("{}:natives-linux", lib.name);
                                let mut native_classifier_path = resolve_library_path(
                                    &native_classifier_name,
                                    &options.mc_data_path,
                                );
                                if !native_classifier_path.exists() {
                                    native_classifier_path = resolve_library_path(
                                        &native_classifier_name,
                                        &options.shared_data_path,
                                    );
                                }
                                if native_classifier_path.exists() {
                                    let parts: Vec<&str> = lib.name.split(':').collect();
                                    if parts.len() >= 2 {
                                        let mut artifact = parts[1].to_string();
                                        // Normalize LWJGL artifact name if it's already a native-style name
                                        if artifact.starts_with("lwjgl")
                                            && artifact.contains("-natives-")
                                        {
                                            if let Some(pos) = artifact.find("-natives-") {
                                                artifact = artifact[..pos].to_string();
                                            }
                                        }
                                        let key = format!("{}:{}:natives-linux", parts[0], artifact);
                                        classpath_map.insert(key, native_classifier_path);
                                    }
                                }
                            }

                            // Legacy LWJGL 2: collect native classifier JARs for extraction
                            // (these use the old downloads.classifiers format)
                            if let Some(ref downloads) = lib.downloads {
                                if let Some(ref classifiers) = downloads.classifiers {
                                    if let Some(native_artifact) = classifiers.get("natives-linux")
                                    {
                                        if let Some(ref rel_path) = native_artifact.path {
                                            let mut native_path = options
                                                .mc_data_path
                                                .join("libraries")
                                                .join(rel_path);
                                            if !native_path.exists() {
                                                native_path = options
                                                    .shared_data_path
                                                    .join("libraries")
                                                    .join(rel_path);
                                            }
                                            if native_path.exists() {
                                                native_jar_paths.push(native_path);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Add Minecraft client jar
    let mc_version = instance.minecraft_version.as_deref().unwrap_or("1.21.8");

    let mc_client_jar_primary = options
        .mc_data_path
        .join("libraries")
        .join("com")
        .join("mojang")
        .join("minecraft")
        .join(mc_version)
        .join(format!("minecraft-{}-client.jar", mc_version));

    let mut classpath: Vec<PathBuf> = classpath_map.into_paths();
    if mc_client_jar_primary.exists() {
        classpath.push(mc_client_jar_primary);
    } else {
        let mc_client_jar_fallback = options
            .shared_data_path
            .join("libraries")
            .join("com")
            .join("mojang")
            .join("minecraft")
            .join(mc_version)
            .join(format!("minecraft-{}-client.jar", mc_version));

        if mc_client_jar_fallback.exists() {
            classpath.push(mc_client_jar_fallback);
        }
    }

    let classpath_str = classpath
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(":");

    let minecraft_dir = &instance.minecraft_dir;

    let mut cmd = if instance.feral_gamemode {
        let mut c = Command::new("gamemoderun");
        c.arg(&options.java_path);
        c
    } else {
        Command::new(&options.java_path)
    };

    if instance.discrete_gpu {
        cmd.env("DRI_PRIME", "1");
        cmd.env("__NV_PRIME_RENDER_OFFLOAD", "1");
        cmd.env("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
        cmd.env("__VK_LAYER_NV_optimus", "NVIDIA_only");
    }

    if instance.zink_vulkan {
        cmd.env("MESA_LOADER_DRIVER_OVERRIDE", "zink");
        cmd.env("GALLIUM_DRIVER", "zink");
        cmd.env("ZINK_DESCRIPTORS", "lazy"); // Sometimes helps with stutters
    }

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(minecraft_dir);

    // Extract native libraries from classifier JARs
    let natives_dir = instance.path.join("natives");
    let _ = fs::remove_dir_all(&natives_dir);
    let _ = fs::create_dir_all(&natives_dir);
    for native_jar in &native_jar_paths {
        if let Err(e) = extract_natives_jar(native_jar, &natives_dir) {
            eprintln!(
                "Warning: failed to extract natives from {:?}: {}",
                native_jar, e
            );
        }
    }
    let natives_dir_str = natives_dir.to_string_lossy().to_string();

    // JVM Args
    let min_mem = options.min_memory.min(options.max_memory);
    cmd.arg(format!("-Xmx{}M", options.max_memory));
    cmd.arg(format!("-Xms{}M", min_mem));
    cmd.arg("-Duser.language=en");

    if instance.use_wayland {
        if is_modern_minecraft_version(mc_version) {
            // Modern Minecraft (1.20.6+) uses a custom GLFW fork with Mojang-specific patches (like glfwSetPreeditCallback).
            // Stock system GLFW libraries do not have these patches, resulting in a NullPointerException crash.
            // Modern LWJGL 3.4.0+ bundled GLFW library has native Wayland support built-in; we just tell it to use Wayland.
            cmd.env("GLFW_PLATFORM", "wayland");
        } else {
            let paths = [
                "/usr/lib/glfw-wayland/libglfw.so.3",
                "/usr/lib/glfw-wayland/libglfw.so.3.5",
                "/usr/lib/x86_64-linux-gnu/libglfw.so.3",
                "/usr/lib/x86_64-linux-gnu/libglfw.so",
                "/usr/lib/libglfw.so.3",
                "/usr/lib/libglfw.so",
                "/usr/lib64/libglfw.so.3",
                "/usr/lib64/libglfw.so",
            ];
            let mut found_glfw = false;
            for path in &paths {
                let p = PathBuf::from(path);
                if p.exists() {
                    cmd.arg(format!("-Dorg.lwjgl.glfw.libname={}", path));
                    found_glfw = true;
                    break;
                }
            }
            if !found_glfw {
                cmd.arg("-Dorg.lwjgl.glfw.libname=glfw");
            }
        }
    }

    // Detect Java major version of the selected Java executable
    let java_major_version = crate::backend::runtime::java::probe_java(&options.java_path)
        .as_ref()
        .and_then(|j| crate::backend::runtime::java::get_java_major_version(&j.version))
        .unwrap_or(8); // Default to 8 if unknown

    // Inject optimized JVM/GC flags
    for arg in get_optimized_jvm_args(options.max_memory, java_major_version) {
        cmd.arg(arg);
    }

    // Apply JVM args from version meta (arguments.jvm), with variable substitution
    let assets_dir_for_jvm = if options.mc_data_path.join("assets").exists() {
        options.mc_data_path.join("assets")
    } else {
        options.shared_data_path.join("assets")
    };
    for jvm_arg in &jvm_args_template {
        let resolved = jvm_arg
            .replace("${natives_directory}", &natives_dir_str)
            .replace("${launcher_name}", "obelisk")
            .replace("${launcher_version}", env!("CARGO_PKG_VERSION"))
            .replace("${classpath}", &classpath_str)
            .replace(
                "${library_directory}",
                &options.mc_data_path.join("libraries").to_string_lossy(),
            )
            .replace("${game_directory}", &minecraft_dir.to_string_lossy())
            .replace("${assets_root}", &assets_dir_for_jvm.to_string_lossy());
        cmd.arg(&resolved);
    }

    // Always ensure -Djava.library.path is set for legacy LWJGL 2 (1.12.2 and below).
    // For modern LWJGL 3 (1.13+), natives are on the classpath as separate JARs —
    // setting -Djava.library.path to the wrong/stale folder would make the JVM load
    // old system natives instead of the bundled ones, causing crashes.
    let has_library_path = jvm_args_template
        .iter()
        .any(|a| a.contains("java.library.path"));
    let has_native_classifiers = !native_jar_paths.is_empty();
    if !has_library_path && has_native_classifiers {
        // Only set it when we actually have legacy extracted natives to point to
        cmd.arg(format!("-Djava.library.path={}", natives_dir_str));
    }

    for arg in &options.jvm_args {
        cmd.arg(arg);
    }

    // ForgeWrapper needs these JVM properties to locate the installer JAR,
    // libraries directory, and vanilla Minecraft client JAR.
    if is_forge {
        let libraries_dir = options.mc_data_path.join("libraries");
        cmd.arg(format!(
            "-Dforgewrapper.librariesDir={}",
            libraries_dir.to_string_lossy()
        ));

        // Locate the Forge installer JAR via maven coords
        if let Some(ref forge_ver) = forge_version {
            let installer_name = format!(
                "net.minecraftforge:forge:{}-{}:installer",
                mc_version, forge_ver
            );
            let mut installer_path = resolve_library_path(&installer_name, &options.mc_data_path);
            if !installer_path.exists() {
                installer_path = resolve_library_path(&installer_name, &options.shared_data_path);
            }
            cmd.arg(format!(
                "-Dforgewrapper.installer={}",
                installer_path.to_string_lossy()
            ));
        }

        // Point to the vanilla Minecraft client JAR
        let mc_jar_path = options
            .mc_data_path
            .join("libraries/com/mojang/minecraft")
            .join(mc_version)
            .join(format!("minecraft-{}-client.jar", mc_version));
        let mc_jar_fallback = options
            .shared_data_path
            .join("libraries/com/mojang/minecraft")
            .join(mc_version)
            .join(format!("minecraft-{}-client.jar", mc_version));
        let actual_mc_jar = if mc_jar_path.exists() {
            &mc_jar_path
        } else {
            &mc_jar_fallback
        };
        cmd.arg(format!(
            "-Dforgewrapper.minecraft={}",
            actual_mc_jar.to_string_lossy()
        ));
    }

    if main_class.is_empty() {
        return Err("No main class found in instance components or metadata.".to_string());
    }

    cmd.arg("-cp").arg(classpath_str);
    cmd.arg(main_class);

    // Minecraft Args
    let assets_dir = if options.mc_data_path.join("assets").exists() {
        options.mc_data_path.join("assets")
    } else {
        options.shared_data_path.join("assets")
    };

    let (auth_player_name, auth_uuid, auth_access_token, user_type) = if let Some(account) = &options.account {
        let u_type = match account.account_type {
            crate::backend::auth::microsoft::AccountType::Microsoft => "msa",
            crate::backend::auth::microsoft::AccountType::Offline => "legacy",
        };
        (account.username.as_str(), account.uuid.as_str(), account.access_token.as_str(), u_type)
    } else {
        ("Player", "00000000-0000-0000-0000-000000000000", "0", "legacy")
    };

    let mc_dir_str = minecraft_dir.to_string_lossy();
    let assets_dir_str = assets_dir.to_string_lossy();
    let vars = [
        ("${auth_player_name}", auth_player_name),
        ("${version_name}", mc_version),
        ("${game_directory}", mc_dir_str.as_ref()),
        ("${assets_root}", assets_dir_str.as_ref()),
        ("${assets_index_name}", asset_index_id.as_str()),
        ("${auth_uuid}", auth_uuid),
        ("${auth_access_token}", auth_access_token),
        ("${user_properties}", "{}"),
        ("${user_type}", user_type),
        ("${version_type}", "release"),
    ];

    if game_arg_tokens.is_empty() {
        game_arg_tokens = vec![
            "--username".to_string(), "${auth_player_name}".to_string(),
            "--version".to_string(), "${version_name}".to_string(),
            "--gameDir".to_string(), "${game_directory}".to_string(),
            "--assetsDir".to_string(), "${assets_root}".to_string(),
            "--assetIndex".to_string(), "${assets_index_name}".to_string(),
            "--uuid".to_string(), "${auth_uuid}".to_string(),
            "--accessToken".to_string(), "${auth_access_token}".to_string(),
            "--userType".to_string(), "${user_type}".to_string(),
            "--versionType".to_string(), "${version_type}".to_string(),
        ];
    }

    for token in &game_arg_tokens {
        let resolved = substitute_vars(token, &vars);
        cmd.arg(resolved);
    }

    // Append extra --tweakClass args from Prism Meta (e.g. Forge FMLTweaker)
    for tweaker in &extra_tweakers {
        cmd.arg("--tweakClass");
        cmd.arg(tweaker);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn java process: {}", e))?;

    Ok(child)
}

pub fn check_instance_assets(instance: &Instance, options: &LaunchOptions) -> bool {
    let mc_version = instance.minecraft_version.as_deref().unwrap_or("1.21.8");

    // Check client jar
    let mc_client_jar = options
        .mc_data_path
        .join("libraries")
        .join("com")
        .join("mojang")
        .join("minecraft")
        .join(mc_version)
        .join(format!("minecraft-{}-client.jar", mc_version));

    let mc_client_jar_fallback = options
        .shared_data_path
        .join("libraries")
        .join("com")
        .join("mojang")
        .join("minecraft")
        .join(mc_version)
        .join(format!("minecraft-{}-client.jar", mc_version));

    if !mc_client_jar.exists() && !mc_client_jar_fallback.exists() {
        println!("Missing Minecraft client jar for version {}", mc_version);
        return false;
    }

    // Check mmc-pack.json components
    let pack_path = instance.path.join("mmc-pack.json");
    if let Ok(pack_content) = fs::read_to_string(&pack_path) {
        if let Ok(pack) = serde_json::from_str::<MmcPack>(&pack_content) {
            for component in pack.components {
                let is_critical = component.uid == "net.minecraft"
                    || component.uid.contains("loader")
                    || component.uid.contains("forge")
                    || component.uid.contains("quilt")
                    || component.uid.contains("neoforged");

                let mut meta_path = options
                    .mc_data_path
                    .join("meta")
                    .join(&component.uid)
                    .join(format!("{}.json", component.version));

                if !meta_path.exists() {
                    let fallback = options
                        .shared_data_path
                        .join("meta")
                        .join(&component.uid)
                        .join(format!("{}.json", component.version));
                    if fallback.exists() {
                        meta_path = fallback;
                    }
                }

                if !meta_path.exists() {
                    if is_critical {
                        println!(
                            "Critical component MISSING: {} (tried {:?} and fallback)",
                            component.uid, meta_path
                        );
                        return false;
                    }
                    continue;
                }

                // Check libraries and asset index in meta
                if let Ok(meta_content) = fs::read_to_string(meta_path) {
                    if let Ok(meta) = serde_json::from_str::<ComponentMeta>(&meta_content) {
                        if let Some(idx) = &meta.asset_index {
                            let mut index_path = options
                                .mc_data_path
                                .join("assets")
                                .join("indexes")
                                .join(format!("{}.json", idx.id));
                            if !index_path.exists() {
                                index_path = options
                                    .shared_data_path
                                    .join("assets")
                                    .join("indexes")
                                    .join(format!("{}.json", idx.id));
                            }
                            if !index_path.exists() {
                                println!("Missing asset index: {}", idx.id);
                                return false;
                            }
                        }

                        if let Some(libs) = meta.libraries {
                            for lib in libs {
                                if is_rule_allowed(&lib.rules) {
                                    // If we have download info, only check if it has a main artifact.
                                    // Natives (classifiers) are often handled differently or not strictly required in classpath.
                                    let needs_check = if let Some(downloads) = &lib.downloads {
                                        downloads.artifact.is_some()
                                    } else {
                                        // Legacy/Fabric format usually means it's a jar unless it's a native
                                        lib.url.is_some() && !lib.name.contains("natives")
                                    };

                                    if needs_check {
                                        let mut path =
                                            resolve_library_path(&lib.name, &options.mc_data_path);
                                        if !path.exists() {
                                            path = resolve_library_path(
                                                &lib.name,
                                                &options.shared_data_path,
                                            );
                                        }
                                        if !path.exists() {
                                            println!("Missing library: {}", lib.name);
                                            return false;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    true
}

/// Generates a set of optimized GC, performance, and compatibility JVM flags
/// tailored to the memory allocation and Java major version.
pub fn get_optimized_jvm_args(max_memory_mb: u32, java_major_version: u32) -> Vec<String> {
    let mut args = Vec::new();

    // 1. Core safety option to ignore unrecognized flags across different Java versions
    args.push("-XX:+IgnoreUnrecognizedVMOptions".to_string());

    // 2. High-performance Garbage Collection (GC) options: Aikar's Flags (tailored for G1GC)
    // G1GC is highly recommended for Minecraft when allocating >= 2GB.
    if max_memory_mb >= 2048 {
        args.push("-XX:+UseG1GC".to_string());
        args.push("-XX:+ParallelRefProcEnabled".to_string());
        args.push("-XX:MaxGCPauseMillis=200".to_string());
        args.push("-XX:+UnlockExperimentalVMOptions".to_string());
        args.push("-XX:+AlwaysPreTouch".to_string());
        args.push("-XX:G1NewSizePercent=30".to_string());
        args.push("-XX:G1MaxNewSizePercent=40".to_string());
        args.push("-XX:G1HeapRegionSize=8m".to_string());
        args.push("-XX:G1ReservePercent=20".to_string());
        args.push("-XX:G1HeapWastePercent=5".to_string());
        args.push("-XX:G1MixedGCCountTarget=4".to_string());
        args.push("-XX:InitiatingHeapFraction=15".to_string());
        args.push("-XX:G1MixedGCLiveThresholdPercent=90".to_string());
        args.push("-XX:G1RSetUpdatingPauseTimePercent=5".to_string());
        args.push("-XX:SurvivorRatio=32".to_string());
        args.push("-XX:+PerfDisableSharedMem".to_string());
        args.push("-XX:MaxTenuringThreshold=1".to_string());
    } else {
        // Fallback/SerialGC for lower memory allocations
        args.push("-XX:+UseSerialGC".to_string());
    }

    // 3. Modern Java (9+) module accessibility rules to prevent reflective access crashes in Forge/Fabric
    if java_major_version >= 9 {
        args.push("--add-opens=java.base/java.io=ALL-UNNAMED".to_string());
        args.push("--add-opens=java.base/java.lang=ALL-UNNAMED".to_string());
        args.push("--add-opens=java.base/java.lang.reflect=ALL-UNNAMED".to_string());
        args.push("--add-opens=java.base/java.util=ALL-UNNAMED".to_string());
        args.push("--add-opens=java.base/java.util.concurrent=ALL-UNNAMED".to_string());
    }

    // 4. Performance & Rendering optimizations
    args.push("-Dsun.java2d.opengl=true".to_string());
    args.push("-Dsun.java2d.noddraw=true".to_string());

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_string_with_spaces_and_quotes() {
        let input = r#"--username "Steve The Miner" --gameDir "/home/user/My Instances/1.20" --demo"#;
        let parsed = parse_args_string(input);
        assert_eq!(parsed, vec![
            "--username",
            "Steve The Miner",
            "--gameDir",
            "/home/user/My Instances/1.20",
            "--demo"
        ]);
    }

    #[test]
    fn test_substitute_vars() {
        let vars = [
            ("${username}", "Alex"),
            ("${game_dir}", "/home/user/.minecraft"),
        ];
        assert_eq!(substitute_vars("--user=${username}", &vars), "--user=Alex");
        assert_eq!(substitute_vars("${game_dir}", &vars), "/home/user/.minecraft");
    }

    #[test]
    fn test_ordered_classpath_preserves_order() {
        let mut cp = OrderedClasspath::default();
        cp.insert("a:lib1".to_string(), PathBuf::from("/lib1.jar"));
        cp.insert("b:lib2".to_string(), PathBuf::from("/lib2.jar"));
        cp.insert("c:lib3".to_string(), PathBuf::from("/lib3.jar"));
        // Re-inserting existing key updates value in-place without changing position
        cp.insert("b:lib2".to_string(), PathBuf::from("/lib2-updated.jar"));

        let paths = cp.into_paths();
        assert_eq!(paths, vec![
            PathBuf::from("/lib1.jar"),
            PathBuf::from("/lib2-updated.jar"),
            PathBuf::from("/lib3.jar"),
        ]);
    }
}
