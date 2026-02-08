#!/usr/bin/env python3
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


RESPONSE_BODY = b"upstream-ok"


class Handler(BaseHTTPRequestHandler):
    def _write_response(self) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(RESPONSE_BODY)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(RESPONSE_BODY)

    def do_GET(self) -> None:
        self._write_response()

    def do_POST(self) -> None:
        self._write_response()

    def do_PUT(self) -> None:
        self._write_response()

    def do_PATCH(self) -> None:
        self._write_response()

    def do_DELETE(self) -> None:
        self._write_response()

    def do_OPTIONS(self) -> None:
        self._write_response()

    def do_HEAD(self) -> None:
        self._write_response()

    def log_message(self, _format: str, *_args: object) -> None:
        return


def parse_bind(bind: str) -> tuple[str, int]:
    host, sep, port_text = bind.rpartition(":")
    if not sep:
        raise ValueError(f"invalid bind address: {bind}")
    return host, int(port_text)


def main() -> None:
    parser = argparse.ArgumentParser(description="Simple upstream echo server")
    parser.add_argument("--bind", default="127.0.0.1:18085")
    args = parser.parse_args()

    host, port = parse_bind(args.bind)
    server = ThreadingHTTPServer((host, port), Handler)
    server.serve_forever()


if __name__ == "__main__":
    main()
