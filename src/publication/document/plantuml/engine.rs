//! One bounded local pipe shared by the documentors of a publication build.
use {
    super::runner,
    anyhow::{Context, Result, ensure},
    std::{
        ffi::{OsStr, OsString},
        time::Duration,
    },
    tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        process::{Child, ChildStdin, ChildStdout},
        sync::Mutex,
        task::JoinHandle,
    },
};
const END: &[u8] = b"GRAPHARCH-PLANTUML-END\n";
const REQUESTS_PER_PROCESS: usize = 128;

pub(crate) struct LocalRenderer {
    program: OsString,
    state:   Mutex<State>,
}
#[derive(Default)]
struct State {
    version: Option<String>,
    session: Option<Session>,
}
struct Session {
    child:    Child,
    input:    ChildStdin,
    output:   BufReader<ChildStdout>,
    errors:   JoinHandle<Result<Vec<u8>>>,
    requests: usize,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.errors.abort();
        // kill_on_drop also covers timeout/error paths; no detached JVM
        // survives.
        let _ = self.child.start_kill();
    }
}
impl LocalRenderer {
    pub(crate) fn new(program: &OsStr) -> Self {
        Self {
            program: program.to_owned(),
            state:   Mutex::new(State::default()),
        }
    }

    pub(crate) async fn version(&self) -> Result<String> {
        let mut state = self.state.lock().await;
        if state.version.is_none() {
            state.version = Some(runner::version(&self.program).await?);
        }
        Ok(state.version.as_ref().expect("initialized version").clone())
    }

    pub(crate) async fn svg(&self, source: &str) -> Result<Vec<u8>> {
        ensure!(
            source.len() <= 128 * 1024,
            "PlantUML source exceeds 128 KiB"
        );
        ensure!(
            !source.lines().any(|line| line.starts_with("@@@")),
            "PlantUML pipe protocol commands are not diagram content"
        );
        ensure!(
            source
                .lines()
                .filter(|line| line.starts_with("@start"))
                .count() ==
                1 &&
                source
                    .lines()
                    .filter(|line| line.starts_with("@end"))
                    .count() ==
                    1,
            "PlantUML pipe accepts exactly one diagram per request"
        );
        let version = self.version().await?;
        let mut state = self.state.lock().await;
        if state
            .session
            .as_ref()
            .is_some_and(|session| session.requests == REQUESTS_PER_PROCESS)
        {
            state.session = None;
        }
        if state.session.is_none() {
            ensure!(
                runner::version(&self.program).await? == version,
                "PlantUML version changed during the publication build"
            );
            state.session = Some(Session::new(&self.program)?);
        }
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            state
                .session
                .as_mut()
                .expect("initialized pipe")
                .svg(source),
        )
        .await
        .context("PlantUML exceeded its 30-second rendering budget")
        .and_then(|value| value);
        if result.is_err() {
            state.session = None;
        }
        result
    }
}
impl Session {
    fn new(program: &OsStr) -> Result<Self> {
        let mut command = runner::command(program, &[
            "-pipe",
            "-tsvg",
            "-charset",
            "UTF-8",
            "-nometadata",
            "-pipeNoStderr",
            "-pipedelimitor",
            "GRAPHARCH-PLANTUML-END",
        ]);
        command.env("JAVA_TOOL_OPTIONS", "-Xmx256m");
        let mut child = command.spawn().context(
            "cannot start the explicitly installed local PlantUML renderer",
        )?;
        let input = child.stdin.take().expect("piped input");
        let output = BufReader::new(child.stdout.take().expect("piped output"));
        let errors = child.stderr.take().expect("piped diagnostics");
        let errors = tokio::spawn(async move {
            let mut bytes = Vec::new(); // bounded: 64 KiB diagnostics plus overflow sentinel.
            errors.take(64 * 1024 + 1).read_to_end(&mut bytes).await?;
            ensure!(
                bytes.len() <= 64 * 1024,
                "PlantUML diagnostics exceed 64 KiB"
            );
            Ok(bytes)
        });
        Ok(Self { child, input, output, errors, requests: 0 })
    }

    async fn svg(&mut self, source: &str) -> Result<Vec<u8>> {
        self.input.write_all(source.as_bytes()).await?;
        if !source.ends_with('\n') {
            self.input.write_all(b"\n").await?;
        }
        self.input.flush().await?;
        let limit =
            crate::publication::image_assets::MAX_IMAGE_BYTES + END.len();
        let mut bytes = Vec::new(); // bounded: one retained SVG, terminator and overflow sentinel.
        while !bytes.ends_with(END) {
            let remaining = limit.saturating_sub(bytes.len()) + 1;
            let count = (&mut self.output)
                .take(remaining as u64)
                .read_until(b'\n', &mut bytes)
                .await?;
            ensure!(
                count > 0,
                "PlantUML closed its pipe before completing the diagram"
            );
            ensure!(
                bytes.len() <= limit,
                "PlantUML process output exceeds byte budget"
            );
        }
        bytes.truncate(bytes.len() - END.len());
        ensure!(
            !bytes.starts_with(b"ERROR"),
            "PlantUML rejected the diagram: {}",
            String::from_utf8_lossy(&bytes)
        );
        let svg = runner::clean_svg(&bytes)?;
        crate::publication::image_assets::validate(&svg, "image/svg+xml")?;
        self.requests += 1;
        Ok(svg)
    }
}
