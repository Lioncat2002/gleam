use crate::{
    error::{Error, FileIOAction, FileKind, GleamExpect, InvalidProjectNameReason},
    NewOptions, Result,
};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use strum_macros::{Display, EnumString, EnumVariantNames};

const GLEAM_STDLIB_VERSION: &'static str = "0.13.0";
const GLEAM_OTP_VERSION: &'static str = "0.1.0";
const ERLANG_OTP_VERSION: &'static str = "22.1";
const PROJECT_VERSION: &'static str = "1.0.0";

#[derive(Debug, Serialize, Deserialize, Display, EnumString, EnumVariantNames, Clone, Copy)]
#[strum(serialize_all = "kebab_case")]
pub enum Template {
    Lib,
    App,
}

#[derive(Debug)]
pub struct Creator {
    root: PathBuf,
    src: PathBuf,
    test: PathBuf,
    github: PathBuf,
    workflows: PathBuf,
    gleam_version: &'static str,
    options: NewOptions,
}

impl Creator {
    fn new(options: NewOptions, gleam_version: &'static str) -> Self {
        let root = match options.project_root {
            Some(ref root) => PathBuf::from(root),
            None => PathBuf::from(&options.name),
        };
        let src = root.join("src");
        let test = root.join("test");
        let github = root.join(".github");
        let workflows = github.join("workflows");
        Self {
            root,
            src,
            test,
            github,
            workflows,
            gleam_version,
            options,
        }
    }

    fn run(&self) -> Result<()> {
        crate::fs::mkdir(&self.root)?;
        crate::fs::mkdir(&self.src)?;
        crate::fs::mkdir(&self.test)?;
        crate::fs::mkdir(&self.github)?;
        crate::fs::mkdir(&self.workflows)?;

        match self.options.template {
            Template::Lib => {
                self.gitignore()?;
                self.github_ci()?;
                self.readme()?;
                self.gleam_toml()?;
                self.lib_rebar_config()?;
                self.app_src()?;
                self.src_module()?;
                self.test_module()?;
            }
            Template::App => {
                crate::fs::mkdir(&self.src.join(&self.options.name))?;
                self.gitignore()?;
                self.github_ci()?;
                self.readme()?;
                self.gleam_toml()?;
                self.app_rebar_config()?;
                self.app_src()?;
                self.src_module()?;
                self.src_application_module()?;
                self.test_module()?;
            }
        }

        Ok(())
    }

    fn src_application_module(&self) -> Result<()> {
        write(
            self.src.join(&self.options.name).join("application.gleam"),
            r#"import gleam/otp/supervisor.{ApplicationStartMode, ErlangStartResult}
import gleam/dynamic.{Dynamic}

fn init(children) {
  children
}

pub fn start(
  _mode: ApplicationStartMode,
  _args: List(Dynamic),
) -> ErlangStartResult {
  init
  |> supervisor.start
  |> supervisor.to_erlang_start_result
}

pub fn stop(_state: Dynamic) {
  supervisor.application_stopped()
}
"#,
        )
    }

    fn src_module(&self) -> Result<()> {
        write(
            self.src.join(format!("{}.gleam", self.options.name)),
            &format!(
                r#"pub fn hello_world() -> String {{
  "Hello, from {}!"
}}
"#,
                self.options.name
            ),
        )
    }

    fn lib_rebar_config(&self) -> Result<()> {
        write(
            self.root.join("rebar.config"),
            &format!(
                r#"{{erl_opts, [debug_info]}}.
{{src_dirs, ["src", "gen/src"]}}.

{{profiles, [
    {{test, [{{src_dirs, ["src", "test", "gen/src", "gen/test"]}}]}}
]}}.

{{project_plugins, [rebar_gleam]}}.

{{deps, [
    {{gleam_stdlib, "{stdlib}"}}
]}}.
"#,
                stdlib = GLEAM_STDLIB_VERSION,
            ),
        )
    }

    fn app_rebar_config(&self) -> Result<()> {
        write(
            self.root.join("rebar.config"),
            &format!(
                r#"{{erl_opts, [debug_info]}}.
{{src_dirs, ["src", "gen/src"]}}.

{{profiles, [
    {{test, [{{src_dirs, ["src", "test", "gen/src", "gen/test"]}}]}}
]}}.

{{shell, [
    % {{config, "config/sys.config"}},
    {{apps, [{name}]}}
]}}.

{{project_plugins, [rebar_gleam]}}.

{{deps, [
    {{gleam_stdlib, "{stdlib}"}},
    {{gleam_otp, "{otp}"}}
]}}.
"#,
                name = self.options.name,
                stdlib = GLEAM_STDLIB_VERSION,
                otp = GLEAM_OTP_VERSION,
            ),
        )
    }

    fn app_src(&self) -> Result<()> {
        let module = match self.options.template {
            Template::App => format!("\n  {{mod, {{{}@application, []}}}},", self.options.name),
            _ => "".to_string(),
        };

        write(
            self.src.join(format!("{}.app.src", self.options.name)),
            &format!(
                r#"{{application, {},
 [{{description, "{}"}},
  {{vsn, "{}"}},
  {{registered, []}},{}
  {{applications,
   [kernel,
    stdlib,
    gleam_stdlib
   ]}},
  {{env,[]}},
  {{modules, []}},

  {{include_files, ["gleam.toml", "gen"]}},
  {{licenses, ["Apache 2.0"]}},
  {{links, []}}
]}}.
"#,
                self.options.name, PROJECT_VERSION, &self.options.description, module,
            ),
        )
    }

    fn gitignore(&self) -> Result<()> {
        write(
            self.root.join(".gitignore"),
            "*.beam
*.iml
*.o
*.plt
*.swo
*.swp
*~
.erlang.cookie
.eunit
.idea
.rebar
.rebar3
_*
_build
docs
ebin
erl_crash.dump
gen
log
logs
rebar3.crashdump
",
        )
    }

    fn readme(&self) -> Result<()> {
        write(
            self.root.join("README.md"),
            &format!(
                r#"# {name}

{description}

## Quick start

```sh
# Build the project
rebar3 compile

# Run the eunit tests
rebar3 eunit

# Run the Erlang REPL
rebar3 shell
```

## Installation

If [available in Hex](https://www.rebar3.org/docs/dependencies#section-declaring-dependencies)
this package can be installed by adding `{name}` to your `rebar.config` dependencies:

```erlang
{{deps, [
    {name}
]}}.
```
"#,
                name = self.options.name,
                description = self.options.description
            ),
        )
    }

    fn github_ci(&self) -> Result<()> {
        write(
            self.workflows.join("test.yml"),
            &format!(
                r#"name: test

on:
  push:
    branches:
      - master
      - main
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2.0.0
      - uses: gleam-lang/setup-erlang@v1.1.0
        with:
          otp-version: {}
      - uses: gleam-lang/setup-gleam@v1.0.2
        with:
          gleam-version: {}
      - run: rebar3 install_deps
      - run: rebar3 eunit
      - run: gleam format --check src test
"#,
                ERLANG_OTP_VERSION, self.gleam_version
            ),
        )
    }

    fn gleam_toml(&self) -> Result<()> {
        write(
            self.root.join("gleam.toml"),
            &format!(
                r#"name = "{}"

# [docs]
# links = [
#   {{ title = 'GitHub', href = 'https://github.com/username/project_name' }}
# ]
"#,
                self.options.name,
            ),
        )
    }

    fn test_module(&self) -> Result<()> {
        write(
            self.test.join(format!("{}_test.gleam", self.options.name)),
            &format!(
                r#"import {name}
import gleam/should

pub fn hello_world_test() {{
  {name}.hello_world()
  |> should.equal("Hello, from {name}!")
}}
"#,
                name = self.options.name
            ),
        )
    }
}

pub fn create(options: NewOptions, version: &'static str) -> Result<()> {
    validate_name(&options.name)?;
    let creator = Creator::new(options, version);
    creator.run()?;

    // write files

    // Print success message
    println!(
        "
Your Gleam project \"{}\" has been successfully created.
The rebar3 program can be used to compile and test it.

    cd {}
    rebar3 eunit
",
        creator.options.name,
        creator.root.to_str().expect("Unable to display path")
    );
    Ok(())
}

fn write(path: PathBuf, contents: &str) -> Result<()> {
    println!(
        "* creating {}",
        path.to_str().expect("Unable to display write path")
    );
    let mut f = File::create(&*path).map_err(|err| Error::FileIO {
        kind: FileKind::File,
        path: path.clone(),
        action: FileIOAction::Create,
        err: Some(err.to_string()),
    })?;

    f.write_all(contents.as_bytes())
        .map_err(|err| Error::FileIO {
            kind: FileKind::File,
            path,
            action: FileIOAction::WriteTo,
            err: Some(err.to_string()),
        })?;
    Ok(())
}

fn validate_name(name: &str) -> Result<(), Error> {
    if crate::erl::is_erlang_reserved_word(name) {
        Err(Error::InvalidProjectName {
            name: name.to_string(),
            reason: InvalidProjectNameReason::ErlangReservedWord,
        })
    } else if crate::erl::is_erlang_standard_library_module(name) {
        Err(Error::InvalidProjectName {
            name: name.to_string(),
            reason: InvalidProjectNameReason::ErlangStandardLibraryModule,
        })
    } else if crate::parse::lexer::str_to_keyword(name).is_some() {
        Err(Error::InvalidProjectName {
            name: name.to_string(),
            reason: InvalidProjectNameReason::GleamReservedWord,
        })
    } else if !regex::Regex::new("^[a-z_]+$")
        .gleam_expect("new name regex could not be compiled")
        .is_match(name)
    {
        Err(Error::InvalidProjectName {
            name: name.to_string(),
            reason: InvalidProjectNameReason::Format,
        })
    } else {
        Ok(())
    }
}
