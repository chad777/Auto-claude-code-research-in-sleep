#!/usr/bin/env python3
"""YouTube Research Tool — Fetch transcripts, metadata, and search videos.

Uses yt-dlp for transcript extraction and YouTube Data API for search.
Falls back to youtube-transcript-api for simple transcript fetching.

Usage:
    python youtube_fetch.py search "query" --max 5
    python youtube_fetch.py info "https://youtube.com/watch?v=..."
    python youtube_fetch.py transcript "https://youtube.com/watch?v=..."
    python youtube_fetch.py search-transcript "query" --max 3
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import urllib.parse
from typing import Any
import urllib.request

YTDLP_INSTALL_MSG = (
    "yt-dlp not found. Install with: pip install yt-dlp\n"
    "Or: winget install yt-dlp"
)

# ---- Helpers ----

def ensure_ytdlp() -> str:
    """Check yt-dlp is available, return path."""
    yt = _which("yt-dlp")
    if yt:
        return yt
    raise RuntimeError(YTDLP_INSTALL_MSG)


def _which(cmd: str) -> str | None:
    """Cross-platform 'which'."""
    for dir_candidate in os.environ.get("PATH", "").split(os.pathsep):
        full = os.path.join(dir_candidate, cmd)
        if os.path.isfile(full) and os.access(full, os.X_OK):
            return full
        full_exe = full + ".exe"
        if os.path.isfile(full_exe) and os.access(full_exe, os.X_OK):
            return full_exe
    return None


def extract_video_id(url_or_id: str) -> str | None:
    """Extract YouTube video ID from URL or return raw ID."""
    patterns = [
        r"(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/embed/|youtube\.com/shorts/)([A-Za-z0-9_-]{11})",
        r"^([A-Za-z0-9_-]{11})$",
    ]
    for p in patterns:
        m = re.search(p, url_or_id)
        if m:
            return m.group(1)
    return None


def clean_transcript(text: str) -> str:
    """Clean raw transcript text."""
    # Remove timestamps like [00:00:00]
    text = re.sub(r"\[\d{2}:\d{2}:\d{2}\]", "", text)
    # Remove multiple spaces/newlines
    text = re.sub(r"\s+", " ", text).strip()
    return text


# ---- Core Functions ----

def search_videos(query: str, max_results: int = 5) -> list[dict[str, Any]]:
    """Search YouTube via yt-dlp's search."""
    yt = ensure_ytdlp()
    search_query = f"ytsearch{max_results}:{query}"

    try:
        result = subprocess.run(
            [yt, "--dump-json", "--flat-playlist", "--no-warnings", search_query],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode != 0:
            return [{"error": result.stderr.strip()}]

        videos = []
        for line in result.stdout.strip().split("\n"):
            if not line.strip():
                continue
            try:
                data = json.loads(line)
                videos.append({
                    "id": data.get("id", ""),
                    "title": data.get("title", ""),
                    "url": f"https://youtube.com/watch?v={data.get('id', '')}",
                    "channel": data.get("channel", ""),
                    "duration": data.get("duration", 0),
                    "view_count": data.get("view_count", 0),
                    "upload_date": data.get("upload_date", ""),
                    "description": (data.get("description", "") or "")[:500],
                })
            except json.JSONDecodeError:
                continue
        return videos
    except subprocess.TimeoutExpired:
        return [{"error": "yt-dlp search timed out"}]
    except Exception as e:
        return [{"error": str(e)}]


def get_video_info(url_or_id: str) -> dict[str, Any]:
    """Get detailed metadata for a single video."""
    yt = ensure_ytdlp()
    video_id = extract_video_id(url_or_id)
    if not video_id:
        return {"error": f"Could not extract video ID from: {url_or_id}"}

    url = f"https://youtube.com/watch?v={video_id}"
    try:
        result = subprocess.run(
            [yt, "--dump-json", "--no-warnings", url],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode != 0:
            return {"error": result.stderr.strip()}

        data = json.loads(result.stdout)
        return {
            "id": data.get("id", ""),
            "title": data.get("title", ""),
            "url": url,
            "channel": data.get("channel", ""),
            "channel_url": data.get("channel_url", ""),
            "duration": data.get("duration", 0),
            "view_count": data.get("view_count", 0),
            "like_count": data.get("like_count", 0),
            "upload_date": data.get("upload_date", ""),
            "description": (data.get("description", "") or "")[:1000],
            "tags": data.get("tags", []),
            "categories": data.get("categories", []),
            "is_live": data.get("is_live", False),
        }
    except Exception as e:
        return {"error": str(e)}


def get_transcript(url_or_id: str) -> dict[str, Any]:
    """Fetch video transcript using yt-dlp's auto-subs."""
    yt = ensure_ytdlp()
    video_id = extract_video_id(url_or_id)
    if not video_id:
        return {"error": f"Could not extract video ID from: {url_or_id}"}

    url = f"https://youtube.com/watch?v={video_id}"
    tmp_dir = tempfile.mkdtemp()

    try:
        # Try to download subtitle/transcript
        result = subprocess.run(
            [
                yt, "--no-warnings", "--write-auto-subs", "--sub-lang", "en",
                "--skip-download", "--sub-format", "vtt",
                "-o", os.path.join(tmp_dir, "%(id)s.%(ext)s"),
                url,
            ],
            capture_output=True,
            text=True,
            timeout=60,
        )

        # Check for subtitle files
        transcript_text = ""
        for fname in os.listdir(tmp_dir):
            if fname.endswith((".vtt", ".srt", ".ttml")):
                filepath = os.path.join(tmp_dir, fname)
                with open(filepath, "r", encoding="utf-8", errors="replace") as f:
                    raw = f.read()
                transcript_text = clean_transcript(raw)
                break

        # Fallback: try youtube-transcript-api
        if not transcript_text:
            try:
                subprocess.run(
                    [sys.executable, "-m", "pip", "install", "youtube-transcript-api", "-q"],
                    capture_output=True, text=True, timeout=30,
                )
                from youtube_transcript_api import get_transcript as get_yt_transcript
                transcript_list = get_yt_transcript(video_id, languages=["en"])
                transcript_text = " ".join(
                    entry.get("text", "") for entry in transcript_list
                )
                transcript_text = clean_transcript(transcript_text)
            except Exception:
                pass

        # Clean up temp dir
        for fname in os.listdir(tmp_dir):
            try:
                os.remove(os.path.join(tmp_dir, fname))
            except OSError:
                pass
        try:
            os.rmdir(tmp_dir)
        except OSError:
            pass

        if not transcript_text:
            return {
                "error": "No transcript available",
                "video_id": video_id,
            }

        return {
            "video_id": video_id,
            "transcript": transcript_text,
            "length_chars": len(transcript_text),
            "length_words": len(transcript_text.split()),
        }

    except subprocess.TimeoutExpired:
        return {"error": "Transcript fetch timed out", "video_id": video_id}
    except Exception as e:
        return {"error": str(e), "video_id": video_id}


def search_and_transcribe(query: str, max_results: int = 3) -> list[dict[str, Any]]:
    """Search YouTube and get transcripts for top results."""
    videos = search_videos(query, max_results)
    results = []
    for v in videos:
        if "error" in v:
            results.append(v)
            continue
        transcript_data = get_transcript(v["id"])
        v["transcript"] = transcript_data.get("transcript", "")
        v["transcript_error"] = transcript_data.get("error", "")
        results.append(v)
    return results


# ---- CLI Entry Point ----

def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="YouTube Research Tool — Fetch transcripts, metadata, and search."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    # Search
    s = subparsers.add_parser("search", help="Search YouTube videos")
    s.add_argument("query")
    s.add_argument("--max", type=int, default=5, dest="max_results")

    # Info
    i = subparsers.add_parser("info", help="Get video metadata")
    i.add_argument("url_or_id")

    # Transcript
    t = subparsers.add_parser("transcript", help="Get video transcript")
    t.add_argument("url_or_id")

    # Search + transcript
    st = subparsers.add_parser("search-transcript", help="Search and get transcripts")
    st.add_argument("query")
    st.add_argument("--max", type=int, default=3, dest="max_results")

    return parser


def main():
    parser = build_parser()
    args = parser.parse_args()

    if args.command == "search":
        results = search_videos(args.query, args.max_results)
        print(json.dumps(results, indent=2, ensure_ascii=False))

    elif args.command == "info":
        result = get_video_info(args.url_or_id)
        print(json.dumps(result, indent=2, ensure_ascii=False))

    elif args.command == "transcript":
        result = get_transcript(args.url_or_id)
        print(json.dumps(result, indent=2, ensure_ascii=False))

    elif args.command == "search-transcript":
        results = search_and_transcribe(args.query, args.max_results)
        print(json.dumps(results, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
