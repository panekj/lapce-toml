use std::fs::File;

use anyhow::{anyhow, Result};
use flate2::read::GzDecoder;
use lapce_plugin::{
  psp_types::{
    lsp_types::{request::Initialize, DocumentFilter, DocumentSelector, InitializeParams, Url},
    Request,
  },
  register_plugin, Http, LapcePlugin, VoltEnvironment, PLUGIN_RPC,
};
use serde_json::Value;

#[derive(Default)]
struct State {}

register_plugin!(State);

macro_rules! string {
  ( $x:expr ) => {
    String::from($x)
  };
}

const LSP_VERSION: &str = "0.7.1";

fn initialize(params: InitializeParams) -> Result<()> {
  let document_selector: DocumentSelector = vec![DocumentFilter {
    language: Some(string!("toml")),
    pattern: Some(string!("**/*.toml")),
    scheme: None,
  }];
  let mut server_args = vec![string!("lsp"), string!("stdio")];

  if let Some(options) = params.initialization_options.as_ref() {
    if let Some(volt) = options.get("volt") {
      if let Some(args) = volt.get("serverArgs") {
        if let Some(args) = args.as_array() {
          server_args = args.into_iter().map(|s| s.to_string()).collect();
        }
      }

      if let Some(server_path) = volt.get("serverPath") {
        if let Some(server_path) = server_path.as_str() {
          if !server_path.is_empty() {
            let server_uri = Url::parse(&format!("urn:{}", server_path))?;
            PLUGIN_RPC.start_lsp(
              server_uri,
              server_args,
              document_selector,
              params.initialization_options,
            );
            return Ok(());
          }
        }
      }
    }
  }

  let arch = match VoltEnvironment::architecture().as_deref() {
    | Ok("x86_64") => "x86_64",
    | Ok("aarch64") => "aarch64",
    | Ok(v) => return Err(anyhow!("Unsupported ARCH: {}", v)),
    | Err(e) => return Err(anyhow!("Error ARCH: {}", e)),
  };

  let os = match VoltEnvironment::operating_system().as_deref() {
    | Ok("macos") => "darwin",
    | Ok("linux") => "linux",
    | Ok("windows") => "windows",
    | Ok(v) => return Err(anyhow!("Unsupported OS: {}", v)),
    | Err(e) => return Err(anyhow!("Error OS: {}", e)),
  };

  let gz_file = format!("taplo-full-{os}-{arch}.gz");

  let download_url =
    format!("https://github.com/panekj/taplo/releases/download/{LSP_VERSION}/{gz_file}");

  let mut resp = Http::get(&download_url)?;
  PLUGIN_RPC.stderr(&format!("STATUS_CODE: {:?}", resp.status_code));
  let body = resp.body_read_all()?;
  std::fs::write(&gz_file, body)?;
  let mut gz = GzDecoder::new(File::open(&gz_file)?);
  let mut file = File::create("taplo")?;
  std::io::copy(&mut gz, &mut file)?;
  std::fs::remove_file(&gz_file)?;

  let volt_uri = VoltEnvironment::uri()?;
  let server_path = Url::parse(&volt_uri)?.join("taplo")?;

  PLUGIN_RPC.start_lsp(
    server_path,
    server_args,
    document_selector,
    params.initialization_options,
  );

  Ok(())
}

impl LapcePlugin for State {
  fn handle_request(&mut self, _id: u64, method: String, params: Value) {
    #[allow(clippy::single_match)]
    match method.as_str() {
      | Initialize::METHOD => {
        let params: InitializeParams = serde_json::from_value(params).unwrap();
        if let Err(e) = initialize(params) {
          PLUGIN_RPC.stderr(&format!("plugin returned with error: {e}"))
        }
      }
      | _ => {}
    }
  }
}
