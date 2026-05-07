// ═══════════════════════════════════════════
// PROJECT DETECTION
// ═══════════════════════════════════════════

/// Ecosystem detected from project root filesystem signals.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectEcosystem {
    Rust,
    JavaScript,
    Python,
    Go,
    Unknown,
}

impl std::fmt::Display for ProjectEcosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectEcosystem::Rust => write!(f, "Rust"),
            ProjectEcosystem::JavaScript => write!(f, "JavaScript/TypeScript"),
            ProjectEcosystem::Python => write!(f, "Python"),
            ProjectEcosystem::Go => write!(f, "Go"),
            ProjectEcosystem::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Everything codemetrics needs to run tests and coverage for a project automatically.
#[derive(Debug, Clone)]
pub struct ProjectProfile {
    pub ecosystem: ProjectEcosystem,
    pub test_cmd: Vec<String>,
    pub coverage_cmd: Vec<String>,
    pub lcov_path: String,
    pub watch_extensions: Vec<String>,
    pub max_crap: f64,
    pub min_doc: f64,
    pub max_debt: usize,
    pub max_complexity_violations: usize,
}

impl ProjectProfile {
    pub fn is_coverage_available(&self) -> bool {
        !self.coverage_cmd.is_empty()
    }
}

/// Inspect filesystem signals starting at `root` and return a `ProjectProfile`.
/// Falls back to unknown defaults when nothing is detected.
pub fn detect_project(root: &str) -> ProjectProfile {
    let p = std::path::Path::new(root);

    // Rust — Cargo.toml present
    if p.join("Cargo.toml").exists() || std::path::Path::new("Cargo.toml").exists() {
        return ProjectProfile {
            ecosystem: ProjectEcosystem::Rust,
            test_cmd: vec!["cargo".into(), "test".into()],
            coverage_cmd: vec![
                "cargo".into(),
                "llvm-cov".into(),
                "--lcov".into(),
                "--output-path".into(),
                "lcov.info".into(),
            ],
            lcov_path: "lcov.info".into(),
            watch_extensions: vec!["rs".into(), "toml".into()],
            max_crap: 15.0,
            min_doc: 95.0,
            max_debt: 0,
            max_complexity_violations: 0,
        };
    }

    // Go — go.mod present
    if p.join("go.mod").exists() || std::path::Path::new("go.mod").exists() {
        return ProjectProfile {
            ecosystem: ProjectEcosystem::Go,
            test_cmd: vec!["go".into(), "test".into(), "./...".into()],
            coverage_cmd: vec![
                "go".into(),
                "test".into(),
                "-coverprofile=coverage.out".into(),
                "./...".into(),
            ],
            lcov_path: String::new(),
            watch_extensions: vec!["go".into()],
            max_crap: 20.0,
            min_doc: 80.0,
            max_debt: 0,
            max_complexity_violations: 0,
        };
    }

    // Python — pyproject.toml or setup.py present
    if p.join("pyproject.toml").exists()
        || p.join("setup.py").exists()
        || std::path::Path::new("pyproject.toml").exists()
        || std::path::Path::new("setup.py").exists()
    {
        return ProjectProfile {
            ecosystem: ProjectEcosystem::Python,
            test_cmd: vec!["pytest".into()],
            coverage_cmd: vec![
                "pytest".into(),
                "--cov".into(),
                "--cov-report=lcov:lcov.info".into(),
            ],
            lcov_path: "lcov.info".into(),
            watch_extensions: vec!["py".into(), "pyi".into()],
            max_crap: 20.0,
            min_doc: 80.0,
            max_debt: 0,
            max_complexity_violations: 0,
        };
    }

    // JavaScript/TypeScript — package.json present
    if p.join("package.json").exists() || std::path::Path::new("package.json").exists() {
        let has_vitest = p.join("vitest.config.ts").exists()
            || p.join("vitest.config.js").exists()
            || std::path::Path::new("vitest.config.ts").exists();
        let test_cmd = if has_vitest {
            vec!["npx".into(), "vitest".into(), "run".into()]
        } else {
            vec!["npm".into(), "test".into()]
        };
        let coverage_cmd = if has_vitest {
            vec![
                "npx".into(),
                "vitest".into(),
                "run".into(),
                "--coverage".into(),
            ]
        } else {
            vec![
                "npx".into(),
                "jest".into(),
                "--coverage".into(),
                "--coverageReporters=lcov".into(),
            ]
        };
        return ProjectProfile {
            ecosystem: ProjectEcosystem::JavaScript,
            test_cmd,
            coverage_cmd,
            lcov_path: "coverage/lcov.info".into(),
            watch_extensions: vec!["js".into(), "ts".into(), "jsx".into(), "tsx".into()],
            max_crap: 20.0,
            min_doc: 70.0,
            max_debt: 0,
            max_complexity_violations: 0,
        };
    }

    // Fallback
    ProjectProfile {
        ecosystem: ProjectEcosystem::Unknown,
        test_cmd: Vec::new(),
        coverage_cmd: Vec::new(),
        lcov_path: String::new(),
        watch_extensions: vec![
            "rs".into(),
            "py".into(),
            "js".into(),
            "ts".into(),
            "go".into(),
            "java".into(),
            "cpp".into(),
            "c".into(),
        ],
        max_crap: 30.0,
        min_doc: 50.0,
        max_debt: 100,
        max_complexity_violations: 0,
    }
}
