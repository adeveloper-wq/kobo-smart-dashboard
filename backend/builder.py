#!/usr/bin/env python3
"""
Standalone Config JSON Builder for Kobo Dashboard
=================================================
Run this on-demand on your computer or server whenever you want to design,
modify, or preview your dashboard configuration.

Usage:
  python backend/builder.py [--port 8001] [--no-browser]

It starts a temporary server, opens your browser, and saves changes directly to config.json.
When finished, press Ctrl+C or click 'Quit' in the browser to stop.
"""

import sys
import os
import argparse
import webbrowser
from pathlib import Path
from http.server import ThreadingHTTPServer

BACKEND_DIR = Path(__file__).resolve().parent
if str(BACKEND_DIR) not in sys.path:
    sys.path.insert(0, str(BACKEND_DIR))

import server


class BuilderRequestHandler(server.DashboardRequestHandler):
    """Extends standard request handler to add a graceful shutdown endpoint."""

    def do_POST(self):
        url = self.path
        if url == "/api/shutdown":
            self.send_json({"status": "ok", "message": "Builder server shutting down..."})
            def shutdown_server():
                import time
                time.sleep(0.5)
                self.server.shutdown()
            import threading
            threading.Thread(target=shutdown_server, daemon=True).start()
            return
        super().do_POST()


def main():
    parser = argparse.ArgumentParser(description="Standalone Kobo Dashboard Config Builder")
    parser.add_argument("--port", type=int, default=8001, help="Port for the builder tool (default: 8001)")
    parser.add_argument("--host", type=str, default="127.0.0.1", help="Host interface (default: 127.0.0.1)")
    parser.add_argument("--no-browser", action="store_true", help="Don't open browser automatically")
    args = parser.parse_args()

    # Force builder enabled for this standalone session
    server.ENABLE_BUILDER = True

    server_address = (args.host, args.port)
    httpd = ThreadingHTTPServer(server_address, BuilderRequestHandler)
    url = f"http://{args.host}:{args.port}/"

    print("=" * 65)
    print("  Kobo Dashboard Standalone Config Builder")
    print(f"  URL: {url}")
    print("  Save your changes and press Ctrl+C to close when done.")
    print("=" * 65)

    if not args.no_browser:
        try:
            webbrowser.open(url)
        except Exception:
            pass

    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nBuilder stopped.")
    finally:
        httpd.server_close()


if __name__ == "__main__":
    main()
