#!/usr/bin/env python3
"""OpenRouter Multi-Model Debate MCP Server

Calls multiple LLMs in parallel via OpenRouter, then runs an arbiter
model to produce a consensus verdict.

Environment Variables:
    OPENROUTER_API_KEY  - OpenRouter API key (required)
    OPENROUTER_BASE_URL - API base URL (default: https://openrouter.ai/api/v1)

Tools provided:
    debate - Run multi-model debate on research findings
"""

import asyncio
import datetime
import json
import os
import sys
import tempfile
import traceback

import httpx

# ---- Configuration ----
API_KEY = os.environ.get("OPENROUTER_API_KEY", "")
BASE_URL = os.environ.get("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1")
SERVER_NAME = os.environ.get("DEBATE_SERVER_NAME", "openrouter-debate")
CHAT_URL = f"{BASE_URL.rstrip('/')}/chat/completions"

# Default debate panel
DEFAULT_PANEL = {
    "panelists": [
        {
            "name": "Critic",
            "model": "google/gemini-3.1-flash-lite",
            "role": "Critical analyst - find flaws, gaps, and counter-arguments"
        },
        {
            "name": "Synthesizer",
            "model": "mistralai/mistral-medium-3-5",
            "role": "Synthesizer - find connections, patterns, and consensus"
        },
        {
            "name": "DeepDiver",
            "model": "deepseek/deepseek-chat",
            "role": "Deep technical analyst - analyze evidence rigor and methodology"
        }
    ],
    "arbiter": {
        "model": "google/gemini-3.5-flash",
        "name": "Arbiter"
    }
}

# ---- Logging ----
DEBUG_LOG = os.path.join(tempfile.gettempdir(), f"{SERVER_NAME}-mcp-debug.log")

def debug_log(msg):
    try:
        with open(DEBUG_LOG, "a", encoding="utf-8") as f:
            f.write(f"{datetime.datetime.now()}: {msg}\n")
    except Exception:
        pass

# ---- MCP Protocol ----
_use_ndjson = False

def send_response(response):
    global _use_ndjson
    json_str = json.dumps(response, separators=(",", ":"))
    json_bytes = json_str.encode("utf-8")
    if _use_ndjson:
        sys.stdout.buffer.write(json_bytes + b"\n")
    else:
        header = f"Content-Length: {len(json_bytes)}\r\n\r\n".encode("utf-8")
        sys.stdout.buffer.write(header + json_bytes)
    sys.stdout.buffer.flush()

def send_event(event, data):
    """Send a JSON-RPC notification-style event for progress."""
    payload = {
        "jsonrpc": "2.0",
        "method": "notifications/message",
        "params": {
            "type": "event",
            "data": {"event": event, "message": data}
        }
    }
    send_response(payload)

async def call_llm(messages, model, max_tokens=4096, temperature=0.7):
    """Call a single model via OpenRouter."""
    if not API_KEY:
        return {"error": "OPENROUTER_API_KEY not set"}

    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {API_KEY}",
        "HTTP-Referer": "https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep",
        "X-Title": "ARIS-OpenRouter-Debate"
    }

    payload = {
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature
    }

    debug_log(f"Calling {model}...")
    try:
        async with httpx.AsyncClient(timeout=300.0) as client:
            response = await client.post(CHAT_URL, headers=headers, json=payload)
            response.raise_for_status()
            data = response.json()
            content = data["choices"][0]["message"]["content"]
            usage = data.get("usage", {})
            debug_log(f"{model} OK (tokens: {usage.get('total_tokens', '?')})")
            return {"content": content, "model": model, "usage": usage}
    except Exception as e:
        debug_log(f"{model} ERROR: {e}")
        return {"error": str(e), "model": model}

async def run_debate_round(question, findings, panel):
    """Run one debate round: all panelists in parallel, then arbiter."""
    # Step 1: Build the prompt for each panelist
    system_prompt = (
        "You are part of a multi-model research analysis panel. "
        "Your role is to analyze research findings critically and thoroughly. "
        "You must output your analysis as valid JSON with these keys:\n"
        "- findings_analysis: dict mapping each key claim to {supported, confidence, counter_args}\n"
        "- gaps: list of gaps or missing context in the evidence\n"
        "- questions: list of important follow-up questions\n"
        "- overall_assessment: string summary of your assessment\n"
        "- confidence: integer 1-10\n"
    )

    user_prompt = f"""RESEARCH QUESTION: {question}

FINDINGS TO ANALYZE:
{findings}

Analyze the findings above. Be specific, critical, and thorough.
Output ONLY valid JSON following the specified schema."""

    # Step 2: Call all panelists in parallel
    panelist_tasks = []
    for p in panel["panelists"]:
        messages = [
            {"role": "system", "content": f"{system_prompt}\nYour name is {p['name']}. {p['role']}"},
            {"role": "user", "content": user_prompt}
        ]
        panelist_tasks.append(call_llm(messages, p["model"]))

    panelist_results = await asyncio.gather(*panelist_tasks)

    # Step 3: Format panelist outputs for arbiter
    panel_outputs = []
    for i, result in enumerate(panelist_results):
        p = panel["panelists"][i]
        if "error" in result:
            panel_outputs.append(f"\n--- {p['name']} ({p['model']}) ERROR ---\n{result['error']}")
        else:
            panel_outputs.append(f"\n--- {p['name']} ({p['model']}) ---\n{result['content']}")

    panel_text = "\n".join(panel_outputs)

    # Step 4: Arbiter produces consensus
    arbiter_system = (
        "You are the Arbiter. You have received analyses from multiple AI panelists "
        "who reviewed the same research findings. Your job is to:\n"
        "1. Identify where ALL panelists agree -> CONSENSUS\n"
        "2. Identify where they disagree -> DISPUTED\n"
        "3. Produce a final synthesized verdict\n"
        "Output valid JSON with keys: consensus_points, disputed_points, "
        "final_verdict, confidence, recommended_next_steps"
    )

    arbiter_messages = [
        {"role": "system", "content": arbiter_system},
        {"role": "user", "content": f"""RESEARCH QUESTION: {question}

Below are analyses from {len(panel['panelists'])} different AI models.
Analyze and produce a consensus verdict.

{panel_text}

Output ONLY valid JSON."""}
    ]

    arbiter_result = await call_llm(arbiter_messages, panel["arbiter"]["model"])

    return {
        "panelist_results": panelist_results,
        "arbiter_result": arbiter_result,
        "panel_config": panel
    }

async def handle_debate_tool(args):
    """Handle the 'debate' MCP tool call."""
    question = args.get("question", "")
    findings = args.get("findings", "")
    custom_panel = args.get("panel", None)

    if not question:
        return {"error": "question is required"}
    if not findings:
        return {"error": "findings is required"}

    panel = DEFAULT_PANEL
    if custom_panel:
        try:
            if isinstance(custom_panel, str):
                custom_panel = json.loads(custom_panel)
            panel = custom_panel
        except (json.JSONDecodeError, TypeError):
            pass

    debug_log(f"Starting debate: {question[:80]}...")

    try:
        result = await run_debate_round(question, findings, panel)
        return {
            "content": json.dumps(result, indent=2, ensure_ascii=False),
            "panelists_used": [p["model"] for p in panel["panelists"]],
            "arbiter_used": panel["arbiter"]["model"]
        }
    except Exception as e:
        debug_log(f"Debate error: {e}\n{traceback.format_exc()}")
        return {"error": str(e)}

# ---- MCP Server Loop ----
async def main():
    global _use_ndjson

    debug_log(f"=== {SERVER_NAME} MCP Server Starting ===")
    debug_log(f"BASE_URL: {BASE_URL}")
    debug_log(f"API_KEY set: {bool(API_KEY)}")
    debug_log(f"Default panel: {[p['model'] for p in DEFAULT_PANEL['panelists']]}")

    # Detect protocol: read first line
    stdin = sys.stdin.buffer
    line = await asyncio.get_event_loop().run_in_executor(None, stdin.readline)

    try:
        init_msg = json.loads(line.decode("utf-8").strip())
    except (json.JSONDecodeError, UnicodeDecodeError):
        init_msg = None

    if init_msg and init_msg.get("method") == "initialize":
        protocol_version = init_msg.get("params", {}).get("protocolVersion", "2024-11-05")

        # Check capabilities
    # Respond to initialize
        send_response({
            "jsonrpc": "2.0",
            "id": init_msg.get("id"),
            "result": {
                "protocolVersion": protocol_version,
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": "1.0.0"
                }
            }
        })

        # Read the next message (should be initialized notification or tools/list)
        line = await asyncio.get_event_loop().run_in_executor(None, stdin.readline)

    if init_msg and init_msg.get("method") == "notifications/initialized":
        line = await asyncio.get_event_loop().run_in_executor(None, stdin.readline)

    # Handle tools/list
    try:
        msg = json.loads(line.decode("utf-8").strip())
    except:
        msg = None

    if msg and msg.get("method") == "tools/list":
        send_response({
            "jsonrpc": "2.0",
            "id": msg.get("id"),
            "result": {
                "tools": [
                    {
                        "name": "debate",
                        "description": "Run a multi-model debate on research findings. Calls multiple LLMs via OpenRouter in parallel, then an arbiter model produces a consensus verdict.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "question": {
                                    "type": "string",
                                    "description": "The research question or topic being investigated"
                                },
                                "findings": {
                                    "type": "string",
                                    "description": "The raw research findings for panelists to analyze"
                                },
                                "panel": {
                                    "type": "string",
                                    "description": "Optional custom panel config as JSON. Default: Critic (minimax), Synthesizer (gemini), DeepDiver (deepseek), Arbiter (minimax)"
                                }
                            },
                            "required": ["question", "findings"]
                        }
                    }
                ]
            }
        })

        line = await asyncio.get_event_loop().run_in_executor(None, stdin.readline)

    # Main request loop
    while line:
        try:
            msg = json.loads(line.decode("utf-8").strip())
        except:
            line = await asyncio.get_event_loop().run_in_executor(None, stdin.readline)
            continue

        msg_id = msg.get("id")
        method = msg.get("method", "")
        params = msg.get("params", {})

        if method == "tools/call":
            tool_name = params.get("name", "")
            arguments = params.get("arguments", {})

            if tool_name == "debate":
                result = await handle_debate_tool(arguments)
                if "error" in result:
                    send_response({
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "error": {"code": -32000, "message": result["error"]}
                    })
                else:
                    send_response({
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": json.dumps(result, indent=2, ensure_ascii=False)
                                }
                            ]
                        }
                    })
            else:
                send_response({
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "error": {"code": -32601, "message": f"Unknown tool: {tool_name}"}
                })
        elif method == "notifications/initialized":
            pass  # Ack, continue
        else:
            send_response({
                "jsonrpc": "2.0",
                "id": msg_id,
                "error": {"code": -32601, "message": f"Unknown method: {method}"}
            })

        line = await asyncio.get_event_loop().run_in_executor(None, stdin.readline)

if __name__ == "__main__":
    asyncio.run(main())
