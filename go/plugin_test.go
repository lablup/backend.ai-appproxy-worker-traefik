package appproxy_traefik_plugin_go

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"
)

type contentEncodingCase struct {
	name                   string
	contentEncodings       []string
	expectedStatus         int
	expectedForwardedCount int
}

func TestServeHTTPContentEncoding(t *testing.T) {
	cases := []contentEncodingCase{
		{name: "br", contentEncodings: []string{"br"}, expectedStatus: http.StatusUnsupportedMediaType, expectedForwardedCount: 0},
		{name: "BR", contentEncodings: []string{"BR"}, expectedStatus: http.StatusUnsupportedMediaType, expectedForwardedCount: 0},
		{name: "gzip, br", contentEncodings: []string{"gzip, br"}, expectedStatus: http.StatusUnsupportedMediaType, expectedForwardedCount: 0},
		{name: "gzip + br", contentEncodings: []string{"gzip", "br"}, expectedStatus: http.StatusUnsupportedMediaType, expectedForwardedCount: 0},
		{name: "none", contentEncodings: nil, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
		{name: "gzip", contentEncodings: []string{"gzip"}, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
		{name: "deflate", contentEncodings: []string{"deflate"}, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
		{name: "identity", contentEncodings: []string{"identity"}, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			forwardedCount := 0
			next := http.HandlerFunc(func(rw http.ResponseWriter, req *http.Request) {
				forwardedCount++
			})
			handler, err := New(context.Background(), next, &Config{LastUsedMarkerPath: t.TempDir() + "/marker.sock"}, "test")
			if err != nil {
				t.Fatal(err)
			}
			req := httptest.NewRequest(http.MethodPost, "http://localhost/", nil)
			for _, contentEncoding := range tc.contentEncodings {
				req.Header.Add("Content-Encoding", contentEncoding)
			}
			recorder := httptest.NewRecorder()

			handler.ServeHTTP(recorder, req)

			if recorder.Code != tc.expectedStatus {
				t.Errorf("status = %d, want %d", recorder.Code, tc.expectedStatus)
			}
			if forwardedCount != tc.expectedForwardedCount {
				t.Errorf("forwarded %d times, want %d", forwardedCount, tc.expectedForwardedCount)
			}
		})
	}
}
