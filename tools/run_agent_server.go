// Server + chat helpers for run_agent.
package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

func resolveModel(key string, reg map[string]map[string]any) (map[string]any, error) {
	if spec, ok := reg[key]; ok {
		out := map[string]any{}
		for k, v := range spec {
			out[k] = v
		}
		out["key"] = key
		if p, ok := out["path"].(string); ok {
			out["path"] = expandHome(p)
		}
		return out, nil
	}
	path := expandHome(key)
	if strings.HasSuffix(path, ".gguf") {
		if _, err := os.Stat(path); err == nil {
			return map[string]any{
				"key": key, "path": path,
				"alias":    strings.TrimSuffix(filepath.Base(path), ".gguf"),
				"defaults": map[string]any{},
			}, nil
		}
	}
	return nil, fmt.Errorf("unknown model %q", key)
}

func serverUp(base string) bool {
	client := &http.Client{Timeout: time.Second}
	resp, err := client.Get(base + "/v1/models")
	if err != nil {
		return false
	}
	defer resp.Body.Close()
	return resp.StatusCode == 200
}

func ensureServer(path, host string, port, ctx int, reasoning string) (string, error) {
	base := fmt.Sprintf("http://%s:%d", host, port)
	if serverUp(base) {
		return base, nil
	}
	logF, _ := os.Create("/tmp/run-agent-llama.log")
	cmd := exec.Command("llama-server",
		"-m", path,
		"--host", host,
		"--port", strconv.Itoa(port),
		"-c", strconv.Itoa(ctx),
		"-np", "1",
		"--ctx-checkpoints", "0",
		"--reasoning", reasoning,
	)
	cmd.Stdout = logF
	cmd.Stderr = logF
	if err := cmd.Start(); err != nil {
		return "", err
	}
	for i := 0; i < 60; i++ {
		if serverUp(base) {
			return base, nil
		}
		time.Sleep(500 * time.Millisecond)
	}
	return "", fmt.Errorf("llama-server failed to become ready")
}

func chat(base, model, system, user string, temperature float64, maxTokens int) (string, error) {
	body := map[string]any{
		"model": model,
		"messages": []map[string]string{
			{"role": "system", "content": system},
			{"role": "user", "content": user},
		},
		"temperature": temperature,
		"max_tokens":  maxTokens,
	}
	b, _ := json.Marshal(body)
	resp, err := http.Post(base+"/v1/chat/completions", "application/json", bytes.NewReader(b))
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	raw, _ := io.ReadAll(resp.Body)
	if resp.StatusCode != 200 {
		return "", fmt.Errorf("chat %s: %s", resp.Status, string(raw))
	}
	var data struct {
		Choices []struct {
			Message struct {
				Content          string `json:"content"`
				ReasoningContent string `json:"reasoning_content"`
			} `json:"message"`
		} `json:"choices"`
	}
	if err := json.Unmarshal(raw, &data); err != nil {
		return "", err
	}
	if len(data.Choices) == 0 {
		return "", fmt.Errorf("no choices")
	}
	msg := data.Choices[0].Message
	if msg.Content != "" {
		return msg.Content, nil
	}
	return msg.ReasoningContent, nil
}
