#!/usr/bin/env python3
import functools
import http.server
import sys


def usage():
    raise SystemExit("Usage: webview-server.py <port> [--bind ADDRESS]")


if len(sys.argv) < 2:
    usage()

try:
    port = int(sys.argv[1])
except ValueError:
    usage()
if port < 1 or port > 65535:
    usage()

bind = "127.0.0.1"
if len(sys.argv) == 4 and sys.argv[2] == "--bind":
    bind = sys.argv[3]
elif len(sys.argv) != 2:
    usage()


class CodexWebviewHandler(http.server.SimpleHTTPRequestHandler):
    def send_head(self):
        for header in ("If-Modified-Since", "If-None-Match"):
            if header in self.headers:
                del self.headers[header]
        return super().send_head()

    def end_headers(self):
        self.send_header("Cache-Control", "no-store, max-age=0")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()


handler = functools.partial(CodexWebviewHandler, directory=".")
with http.server.ThreadingHTTPServer((bind, port), handler) as httpd:
    httpd.serve_forever()
