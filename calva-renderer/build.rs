use wesl::Wesl;

const SHADERS_PATH: &str = "src/shaders";

fn main() -> anyhow::Result<()> {
    let compiler = Wesl::new(SHADERS_PATH);

    for entry in glob::glob(&format!("{SHADERS_PATH}/**/*.wesl"))? {
        let entrypoint = entry?
            .display()
            .to_string()
            .replace(&format!("{SHADERS_PATH}/"), "");

        let out_name = entrypoint.clone().replace(".wesl", "").replace("/", "::");

        compiler.build_artifact(entrypoint, &out_name);
    }

    Ok(())
}
