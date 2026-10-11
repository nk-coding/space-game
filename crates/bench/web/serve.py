#!/usr/bin/env python3
"""Static server for the benchmark page. Sends COOP/COEP headers so the page is cross-origin isolated
(performance.now() then has ~5 us resolution instead of ~100 us). Usage: python3 web/serve.py [port]"""
import http.server, os, sys

class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, '.wasm': 'application/wasm', '.js': 'text/javascript', '.mjs': 'text/javascript'}

    def end_headers(self):
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        self.send_header('Cache-Control', 'no-store')
        super().end_headers()

if __name__ == '__main__':
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8090
    print(f'serving {os.getcwd()} on http://0.0.0.0:{port}/ (VM LAN address: http://10.77.6.2:{port}/)')
    http.server.ThreadingHTTPServer(('0.0.0.0', port), Handler).serve_forever()
