# OpenTelemetry Integration

## Overview
The terminator MCP agent supports OpenTelemetry (OTLP) for distributed tracing. Telemetry is **only** triggered by the `execute_sequence` tool (used by `terminator mcp run`), not by individual MCP tool calls.

## Configuration

### Environment Variables

- `OTEL_EXPORTER_OTLP_ENDPOINT`: OTLP endpoint (default: `http://localhost:4318`)
  - **Important**: Use port 4318 for HTTP/protobuf, NOT 4317 (gRPC)
- `OTEL_SDK_DISABLED`: Set to `true` to disable telemetry completely
- `OTEL_SKIP_COLLECTOR_CHECK`: Set to `true` to skip collector availability check
- `OTEL_RETRY_DURATION_MINS`: Max minutes to retry connecting to collector (default: 15)
- `OTEL_RETRY_INTERVAL_SECS`: Seconds between retry attempts (default: 30)
- `OTEL_SERVICE_NAME`: Service name for traces (default: `terminator-mcp-agent`)

### Common Issues

#### Protocol Mismatch Error
If you see errors like:
```
"transport: http2Server.HandleStreams received bogus greeting from client: \"POST /v1/traces HTTP/1.1\""
```

This means the agent is sending HTTP/1.1 to a gRPC endpoint. Ensure:
1. `OTEL_EXPORTER_OTLP_ENDPOINT` is set to port 4318 (HTTP), not 4317 (gRPC)
2. Check for global environment variables that might override the default

#### Testing with OpenTelemetry Collector

1. Start an OTLP collector with HTTP endpoint on port 4318:
```yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

exporters:
  file:
    path: telemetry-output.json
  logging:
    loglevel: debug

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: []
      exporters: [file, logging]
```

2. Run MCP agent with telemetry:
```bash
env OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4318" \
    ./terminator-mcp-agent.exe -t http --host 0.0.0.0 -p 8093
```

3. Execute a workflow to generate traces:
```bash
terminator mcp run workflow.yml --url http://localhost:8093/mcp
```

## Telemetry Data

### Workflow Spans
- Name: `execute_sequence`
- Attributes:
  - `workflow.name`: Workflow name
  - `workflow.total_steps`: Number of steps
  - `workflow.stop_on_error`: Error handling mode

### Step Spans
- Name: `step.<tool_name>`
- Attributes:
  - `tool.name`: Tool being executed
  - `step.number`: Step index
  - `step.total`: Total steps in workflow

### Events
- `step.started`: When a step begins execution
- `step.completed`: When a step finishes (with status)
- `workflow.completed`: When workflow finishes