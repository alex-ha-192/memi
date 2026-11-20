"""Moves Trunk's inline start-up script out of dist/index.html into its own file.

The site's Content-Security-Policy (public/_headers) allows scripts from this origin only, never
inline ones, so the page would otherwise stay blank. The file name carries a hash of its contents,
because .js files are cached for a year.
"""
import hashlib
import pathlib
import re
import sys

dist = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "dist")
index = dist / "index.html"
html = index.read_text()
scripts = list(re.finditer(r'<script type="module">(.*?)</script>', html, re.S))
if len(scripts) != 1:
    sys.exit(f"Expected one inline module script in {index}, found {len(scripts)}")
body = scripts[0].group(1).strip() + "\n"
name = f"boot-{hashlib.sha256(body.encode()).hexdigest()[:16]}.js"
(dist / name).write_text(body)
html = html[: scripts[0].start()] + f'<script type="module" src="/{name}"></script>' + html[scripts[0].end():]
index.write_text(html)
if re.search(r"<script(?![^>]*\bsrc=)[^>]*>", html):
    sys.exit("index.html still has an inline script")
print(f"Moved the start-up script to {name}")
