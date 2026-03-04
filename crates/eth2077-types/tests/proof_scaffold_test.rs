use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn parse_lake_package_name(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with("package") {
            return None;
        }

        let first_quote = trimmed.find('"')?;
        let remainder = &trimmed[first_quote + 1..];
        let second_quote = remainder.find('"')?;
        Some(remainder[..second_quote].to_string())
    })
}

fn collect_lean_files(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let entries = fs::read_dir(dir).expect("proofs directory should be readable");

    for entry in entries {
        let path = entry.expect("directory entry should be readable").path();

        if path.is_dir() {
            collect_lean_files(&path, root, out);
            continue;
        }

        let is_lean = path.extension().and_then(|ext| ext.to_str()) == Some("lean");
        if is_lean {
            let rel = path
                .strip_prefix(root)
                .expect("path should be under proofs directory")
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
}

#[test]
fn proof_scaffolding_present() {
    let proofs_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("proofs");

    assert!(
        proofs_dir.exists() && proofs_dir.is_dir(),
        "expected proofs directory at {}",
        proofs_dir.display()
    );

    let lakefile_path = proofs_dir.join("lakefile.lean");
    let lakefile = fs::read_to_string(&lakefile_path).expect("lakefile.lean should be readable");

    let package_name =
        parse_lake_package_name(&lakefile).expect("lakefile.lean should declare a package name");
    assert_eq!(package_name, "eth2077-proofs");

    let mut lean_files = Vec::new();
    collect_lean_files(&proofs_dir, &proofs_dir, &mut lean_files);

    let lean_set: BTreeSet<String> = lean_files.into_iter().collect();
    let expected = [
        "lakefile.lean",
        "ETH2077Proofs.lean",
        "ETH2077Proofs/TestnetGates.lean",
        "ETH2077Proofs/ExecutionOptimization.lean",
        "ETH2077Proofs/PayloadTimeliness.lean",
        "ETH2077Proofs/InclusionList.lean",
        "ETH2077Proofs/OobConsensus.lean",
        "ETH2077Proofs/WitnessIntegrity.lean",
    ];

    for rel in expected {
        assert!(
            lean_set.contains(rel),
            "expected Lean scaffold file missing: {rel}"
        );
    }
}
