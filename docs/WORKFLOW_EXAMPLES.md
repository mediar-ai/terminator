# YAML Workflow Examples

This document showcases simple end-to-end **Terminator** workflows written in YAML that you can run on **Windows** today. Each example is powered by the `execute_sequence` tool exposed by `terminator-mcp-agent`.

> 👉 **How to run**
>
> 1. Make sure the MCP agent is installed and the Terminator workspace is built:
>    ```bash
>    cargo build --release
>    ```
> 2. Execute a workflow (replace the path with the one you want to try):
>    ```bash
>    ./target/release/gist-executor examples/workflows/notepad_type.yaml
>    ```
>    or load the YAML in a Cursor/VS Code MCP session and click *Run*.

---

## 1. `notepad_type.yaml`

Automates **Notepad**:

1. Launches `notepad.exe`.
2. Waits for the edit pane to appear.
3. Types custom text (parameterised via `text_to_type`).
4. Closes Notepad.

```yaml
# abbreviated for readability
steps:
  - open_application (notepad.exe)
  - wait_for_element (edit area)
  - type_into_element ("Hello, Terminator!")
  - close_element (window)
```

---

## 2. `calculator_addition.yaml`

Automates **Windows Calculator**:

1. Launches Calculator.
2. Clicks `7 + 3 =`.
3. Waits until the result display reads **10**.
4. Closes the Calculator window.

```yaml
steps:
  - open_application (calc)
  - click_element (7 → + → 3 → =)
  - wait_for_element (display is 10)
  - close_element (window)
```

---

Feel free to duplicate these files under `examples/workflows/` and iterate on them for your own automation scenarios.

## 3. `x_follow_basic.yaml`

Automates **X.com** (Twitter):

1. Opens https://x.com.
2. Waits until a `Follow` button is visible.
3. Runs **five** scroll + follow cycles:
   - Click the first visible `Follow` button (if any).
   - Scrolls the timeline (`PageDown`).
   - Short delay to let new content load.
4. Optionally closes the browser tab with `{Ctrl}{W}`.

This is a straightforward, unrolled loop—handy for demo purposes.

---

## 4. `x_follow_advanced.yaml`

A slightly more sophisticated variant that:

1. Opens X.com and waits for the feed.
2. Executes three *batch* cycles where each cycle:
   - Attempts to click up to three visible `Follow` buttons (using `continue_on_error` so the workflow keeps going if none are found).
   - Scrolls **twice** (`PageDown`) to load fresh content.
   - Waits 1.5 s between cycles for stability.
3. Closes the tab at the end.

Feel free to duplicate and extend these YAMLs—e.g. increase the number of click attempts per cycle or add more cycles—to suit your own growth-hacking experiments.