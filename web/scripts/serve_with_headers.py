"""Serves a folder with the Content-Security-Policy from its _headers file, like Cloudflare Pages does.

Used by CI to check the built web app starts under the real policy: serve_with_headers.py DIR PORT
"""
import http.server, sys, os
root = sys.argv[1]; port = int(sys.argv[2])
csp = ""
for line in open(os.path.join(root, "_headers")):
    if line.strip().startswith("Content-Security-Policy:"):
        csp = line.split(":", 1)[1].strip(); break
class H(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k): super().__init__(*a, directory=root, **k)
    def end_headers(self):
        self.send_header("Content-Security-Policy", csp); super().end_headers()
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("", port), H).serve_forever()
