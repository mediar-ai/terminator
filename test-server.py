import http.server
import socketserver
import json
from datetime import datetime

PORT = 8080

class TestHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/health':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            response = {
                'status': 'healthy',
                'service': 'remote-ui-automation-test',
                'timestamp': datetime.now().isoformat()
            }
            self.wfile.write(json.dumps(response).encode())
        else:
            super().do_GET()

    def do_POST(self):
        if self.path == '/execute':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)

            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()

            response = {
                'success': True,
                'message': 'Test response from Azure VM',
                'received': json.loads(post_data.decode())
            }
            self.wfile.write(json.dumps(response).encode())

print(f"Starting test server on port {PORT}")
with socketserver.TCPServer(("", PORT), TestHandler) as httpd:
    httpd.serve_forever()
