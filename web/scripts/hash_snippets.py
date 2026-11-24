"""Gives the app's own JavaScript (src/glue.js) a name that changes whenever it does.

Run straight after `trunk build`, before externalise_boot.py. Trunk names its own files after
their contents, but wasm-bindgen puts snippets such as glue.js at a path named after the crate
(`snippets/memi-web-<crate hash>/src/glue.js`), which stays the same from build to build. The
site tells browsers to keep every .js file for a year (public/_headers), so a browser that had
an older build kept its older glue.js; its fingerprint no longer matched the page, the browser
refused it, and the app never started (a white page, from 24 Nov 2026, on browsers that had
opened Memi before).

This renames the snippet folder after a hash of what's in it, then renames Trunk's script that
imports it (its contents change with the new path) and fixes the page's links and fingerprints.
"""
import base64
import hashlib
import pathlib
import re
import shutil
import sys

dist = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "dist")
index_path = dist / "index.html"
index = index_path.read_text()


def sri(data: bytes) -> str:
    return "sha384-" + base64.b64encode(hashlib.sha384(data).digest()).decode()


snippets = dist / "snippets"
if not snippets.is_dir():
    sys.exit(0)  # No snippets in this build: nothing to do.

scripts = sorted(dist.glob("memi-web-*.js"))
if len(scripts) != 1:
    sys.exit(f"Expected one memi-web-*.js in {dist}, found {[p.name for p in scripts]}")
script = scripts[0]
text = script.read_text()

for folder in sorted(p for p in snippets.iterdir() if p.is_dir()):
    digest = hashlib.sha256()
    for f in sorted(q for q in folder.rglob("*") if q.is_file()):
        digest.update(f.relative_to(folder).as_posix().encode())
        digest.update(f.read_bytes())
    new = f"{folder.name}-{digest.hexdigest()[:16]}"
    old_ref, new_ref = f"snippets/{folder.name}/", f"snippets/{new}/"
    # Only the import statements change. The same path also appears as the keys of the imports
    # handed to the WebAssembly, which names its imports by the original path.
    text, n = re.subn(rf"""(\bfrom\s*['"]\.?/){re.escape(old_ref)}""", lambda m: m.group(1) + new_ref, text)
    if n == 0:
        sys.exit(f"{script.name} doesn't import from {old_ref}")
    index = index.replace(old_ref, new_ref)
    shutil.move(folder, snippets / new)

# Trunk's script now has different contents, so it gets a new name too (else a browser could
# keep the old one, which points at the old snippets).
data = text.encode()
new_name = f"memi-web-{hashlib.sha256(data).hexdigest()[:16]}.js"
(dist / new_name).write_bytes(data)
script.unlink()
old_integrity = re.search(rf'href="/{re.escape(script.name)}"[^>]*integrity="([^"]+)"', index)
index = index.replace(f"/{script.name}", f"/{new_name}")
if old_integrity:
    index = index.replace(old_integrity.group(1), sri(data))
# The wasm keeps its name (its contents are unchanged); Trunk's start-up code names it directly.

index_path.write_text(index)
print(f"{script.name} -> {new_name}; snippets: {[p.name for p in snippets.iterdir()]}")
