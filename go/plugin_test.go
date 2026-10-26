package appproxy_traefik_plugin_go

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"
)

type contentEncodingCase struct {
	name                    string
	rejectBrotliRequestBody bool
	contentEncodings        []string
	expectedStatus          int
	expectedForwardedCount  int
}

func TestServeHTTPContentEncoding(t *testing.T) {
	cases := []contentEncodingCase{
		{name: "enabled/br", rejectBrotliRequestBody: true, contentEncodings: []string{"br"}, expectedStatus: http.StatusUnsupportedMediaType, expectedForwardedCount: 0},
		{name: "enabled/BR", rejectBrotliRequestBody: true, contentEncodings: []string{"BR"}, expectedStatus: http.StatusUnsupportedMediaType, expectedForwardedCount: 0},
		{name: "enabled/gzip, br", rejectBrotliRequestBody: true, contentEncodings: []string{"gzip, br"}, expectedStatus: http.StatusUnsupportedMediaType, expectedForwardedCount: 0},
		{name: "enabled/gzip + br", rejectBrotliRequestBody: true, contentEncodings: []string{"gzip", "br"}, expectedStatus: http.StatusUnsupportedMediaType, expectedForwardedCount: 0},
		{name: "enabled/none", rejectBrotliRequestBody: true, contentEncodings: nil, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
		{name: "enabled/gzip", rejectBrotliRequestBody: true, contentEncodings: []string{"gzip"}, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
		{name: "enabled/deflate", rejectBrotliRequestBody: true, contentEncodings: []string{"deflate"}, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
		{name: "enabled/identity", rejectBrotliRequestBody: true, contentEncodings: []string{"identity"}, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
		{name: "disabled/br", rejectBrotliRequestBody: false, contentEncodings: []string{"br"}, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
		{name: "disabled/gzip, br", rejectBrotliRequestBody: false, contentEncodings: []string{"gzip, br"}, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
		{name: "disabled/gzip", rejectBrotliRequestBody: false, contentEncodings: []string{"gzip"}, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
		{name: "disabled/none", rejectBrotliRequestBody: false, contentEncodings: nil, expectedStatus: http.StatusOK, expectedForwardedCount: 1},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			forwardedCount := 0
			next := http.HandlerFunc(func(rw http.ResponseWriter, req *http.Request) {
				forwardedCount++
			})
			config := &Config{
				LastUsedMarkerPath:      t.TempDir() + "/marker.sock",
				RejectBrotliRequestBody: tc.rejectBrotliRequestBody,
			}
			handler, err := New(context.Background(), next, config, "test")
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
