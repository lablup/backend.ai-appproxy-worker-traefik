package appproxy_traefik_plugin_go

import (
	"context"
	"fmt"
	"html/template"
	"net"
	"net/http"
	"os"
	"strings"
)

type Config struct {
	LastUsedMarkerPath string   `yaml:"lastusedmarkerpath"`
	Id                 string   `yaml:"id"`
	SessionIds         []string `yaml:"sessionids"`
}

func CreateConfig() *Config {
	return &Config{
		LastUsedMarkerPath: "",
		Id:                 "",
		SessionIds:         []string{},
	}
}

// Plugin a Plugin plugin.
type Plugin struct {
	config   *Config
	next     http.Handler
	name     string
	template *template.Template
}

// New created a new Plugin plugin.
func New(ctx context.Context, next http.Handler, config *Config, name string) (http.Handler, error) {
	os.Stderr.WriteString(fmt.Sprintf("########## New, config: %+v $$$$$$$$$$\n", config))
	return &Plugin{
		config:   config,
		next:     next,
		name:     name,
		template: template.New("demo").Delims("[[", "]]"),
	}, nil
}

func (a *Plugin) MarkLastUsed(isWebsocket bool) {
	client := http.Client{
		Transport: &http.Transport{
			DialContext: func(_ context.Context, _, _ string) (net.Conn, error) {
				return net.Dial("unix", a.config.LastUsedMarkerPath)
			},
		},
	}

	_, err := client.Head(fmt.Sprintf("http://unix/circuit.%s/mark-last-used-time", a.config.Id))
	if err != nil {
		os.Stderr.WriteString(fmt.Sprintf("Failed to mark last used time of circuit.%s: %v\n", a.config.Id, err))
	}

	for _, sessionId := range a.config.SessionIds {
		_, err := client.Head(fmt.Sprintf("http://unix/session.%s/mark-last-used-time", sessionId))
		if err != nil {
			os.Stderr.WriteString(fmt.Sprintf("Failed to mark last used time of session.%s: %v\n", sessionId, err))
		}
	}

	if isWebsocket {
		_, err := client.Head(fmt.Sprintf("http://unix/%s/mark-active", a.config.Id))
		if err != nil {
			os.Stderr.WriteString(fmt.Sprintf("Failed to mark active of %s: %v\n", a.config.Id, err))
		}
	}
}

func (a *Plugin) MarkInactive() {
	client := http.Client{
		Transport: &http.Transport{
			DialContext: func(_ context.Context, _, _ string) (net.Conn, error) {
				return net.Dial("unix", a.config.LastUsedMarkerPath)
			},
		},
	}

	_, err := client.Head(fmt.Sprintf("http://unix/%s/mark-inactive", a.config.Id))
	if err != nil {
		os.Stderr.WriteString(fmt.Sprintf("Failed to mark inactive of %s: %v\n", a.config.Id, err))
	}
}

func (a *Plugin) ServeHTTP(rw http.ResponseWriter, req *http.Request) {

	isWebsocket := strings.ToLower(req.Header.Get("Upgrade")) == "websocket"

	go a.MarkLastUsed(isWebsocket)
	a.next.ServeHTTP(rw, req)
	if isWebsocket {
		go a.MarkInactive()
	}
}
