import http.server
import json
import os
import threading
import time


TOKEN = os.environ["MELORI_ENGINE_TOKEN"]
PORT = int(os.environ["MELORI_ENGINE_PORT"])

dump = os.environ.get("FAKE_ENV_DUMP")
if dump:
    with open(dump, "a", encoding="utf-8") as handle:
        handle.write(os.environ.get("MELORI_LLM_MODEL", "") + "\n")


def exit_later():
    value = os.environ.get("FAKE_EXIT_AFTER")
    if value:
        time.sleep(float(value))
        os._exit(0)


delay = float(os.environ.get("FAKE_DELAY_START", "0"))
if delay:
    time.sleep(delay)
threading.Thread(target=exit_later, daemon=True).start()


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self):
        if self.path != "/health" or self.headers.get("X-Melori-Token") != TOKEN:
            self.send_response(401)
            self.end_headers()
            return
        body = json.dumps({"ok": True, "disk_encryption": "unknown"}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
