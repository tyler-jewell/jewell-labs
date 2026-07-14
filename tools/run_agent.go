// Minimal agent runner (Go): agents/{name}.md → llama-server chat.
// Build: go build -o tools/run_agent_go tools/run_agent.go tools/run_agent_frontmatter.go tools/run_agent_server.go
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

func main() {
	dryParse, serveOnly := false, false
	modelOverride := ""
	var positional []string
	args := os.Args[1:]
	for i := 0; i < len(args); i++ {
		a := args[i]
		switch a {
		case "--dry-parse":
			dryParse = true
		case "--serve-only":
			serveOnly = true
		case "--model":
			i++
			if i < len(args) {
				modelOverride = args[i]
			}
		default:
			if strings.HasPrefix(a, "-") {
				fmt.Fprintf(os.Stderr, "unknown flag: %s\n", a)
				os.Exit(1)
			}
			positional = append(positional, a)
		}
	}
	if len(positional) == 0 {
		fmt.Fprintln(os.Stderr, "usage: run_agent_go <agent> [prompt] [--dry-parse] [--serve-only] [--model KEY]")
		os.Exit(1)
	}
	agent := positional[0]
	prompt := `What is 2+2? Put answer in \boxed{}.`
	if len(positional) > 1 {
		prompt = strings.Join(positional[1:], " ")
	}

	root := rootDir()
	agentPath := filepath.Join(root, "agents", agent+".md")
	raw, err := os.ReadFile(agentPath)
	if err != nil {
		fmt.Fprintln(os.Stderr, "missing", agentPath)
		os.Exit(1)
	}
	fm, body := parseFrontmatter(string(raw))
	modelKey := modelOverride
	if modelKey == "" {
		if v, ok := fm["model"]; ok && v != nil {
			modelKey = asString(v, "")
		}
	}
	if modelKey == "" {
		modelKey = asString(fm["default_model"], "")
	}
	if modelKey == "" {
		fmt.Fprintln(os.Stderr, "missing default_model")
		os.Exit(1)
	}
	reg := loadRegistry(filepath.Join(root, "models", "registry.yaml"))
	spec, err := resolveModel(modelKey, reg)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	server, _ := fm["server"].(map[string]any)
	if server == nil {
		server = map[string]any{}
	}
	sampling, _ := fm["sampling"].(map[string]any)
	if sampling == nil {
		sampling = map[string]any{}
	}
	defaults, _ := spec["defaults"].(map[string]any)
	if defaults == nil {
		defaults = map[string]any{}
	}
	host := asString(server["host"], "127.0.0.1")
	port := asInt(server["port"], 8080)
	ctx := asInt(server["ctx"], asInt(defaults["ctx"], 2048))
	reasoning := asString(server["reasoning"], asString(defaults["reasoning"], "off"))
	temp := 0.0
	if sampling["temperature"] != nil {
		switch t := sampling["temperature"].(type) {
		case float64:
			temp = t
		case int:
			temp = float64(t)
		}
	}
	maxTokens := asInt(sampling["max_tokens"], 256)

	if dryParse {
		out := map[string]any{
			"agent":        agent,
			"model_key":    modelKey,
			"path":         spec["path"],
			"system_chars": len(body),
			"port":         port,
		}
		_ = json.NewEncoder(os.Stdout).Encode(out)
		return
	}

	base, err := ensureServer(asString(spec["path"], ""), host, port, ctx, reasoning)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	if serveOnly {
		fmt.Println(base)
		return
	}
	alias := asString(spec["alias"], "local")
	text, err := chat(base, alias, body, prompt, temp, maxTokens)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	fmt.Println(text)
}
