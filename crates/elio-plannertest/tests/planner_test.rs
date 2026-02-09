use std::path::Path;

fn main() -> anyhow::Result<()> {
    sqlplannertest::planner_test_runner(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"), || async {
        Ok(elio_plannertest::test_env::TestEnv::default())
    })?;
    Ok(())
}
