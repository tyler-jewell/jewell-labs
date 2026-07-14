// Frontmatter + registry parsing for run_agent.
package main

import (
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
)

func rootDir() string {
	exe, err := os.Executable()
	if err == nil {
		if d := filepath.Dir(exe); strings.HasSuffix(d, "tools") || filepath.Base(d) == "tools" {
			return filepath.Dir(d)
		}
	}
	wd, _ := os.Getwd()
	if _, err := os.Stat(filepath.Join(wd, "agents")); err == nil {
		return wd
	}
	if _, err := os.Stat(filepath.Join(wd, "..", "agents")); err == nil {
		return filepath.Clean(filepath.Join(wd, ".."))
	}
	return wd
}

func parseFrontmatter(text string) (map[string]any, string) {
	if !strings.HasPrefix(text, "---") {
		return map[string]any{}, text
	}
	parts := strings.SplitN(text, "---", 3)
	if len(parts) < 3 {
		return map[string]any{}, text
	}
	raw, body := parts[1], strings.TrimPrefix(parts[2], "\n")
	data := map[string]any{}
	type frame struct {
		indent int
		m      map[string]any
	}
	stack := []frame{{0, data}}
	re := regexp.MustCompile(`^(\s*)([A-Za-z0-9_]+):\s*(.*)$`)
	for _, line := range strings.Split(raw, "\n") {
		if strings.TrimSpace(line) == "" || strings.HasPrefix(strings.TrimSpace(line), "#") {
			continue
		}
		m := re.FindStringSubmatch(line)
		if m == nil {
			continue
		}
		indent := len(m[1])
		key, val := m[2], strings.TrimSpace(m[3])
		for len(stack) > 1 && indent <= stack[len(stack)-1].indent {
			stack = stack[:len(stack)-1]
		}
		cur := stack[len(stack)-1].m
		if val == "" {
			child := map[string]any{}
			cur[key] = child
			stack = append(stack, frame{indent, child})
			continue
		}
		if (strings.HasPrefix(val, `"`) && strings.HasSuffix(val, `"`)) ||
			(strings.HasPrefix(val, `'`) && strings.HasSuffix(val, `'`)) {
			val = val[1 : len(val)-1]
		}
		var v any = val
		switch {
		case val == "true":
			v = true
		case val == "false":
			v = false
		case val == "null":
			v = nil
		default:
			if i, err := strconv.Atoi(val); err == nil {
				v = i
			} else if f, err := strconv.ParseFloat(val, 64); err == nil {
				v = f
			}
		}
		cur[key] = v
	}
	return data, body
}

func loadRegistry(path string) map[string]map[string]any {
	b, err := os.ReadFile(path)
	if err != nil {
		return map[string]map[string]any{}
	}
	models := map[string]map[string]any{}
	var cur string
	inDefaults := false
	reModel := regexp.MustCompile(`^  ([A-Za-z0-9_.-]+):\s*$`)
	reField := regexp.MustCompile(`^    ([A-Za-z0-9_]+):\s*(.+)$`)
	reDef := regexp.MustCompile(`^      ([A-Za-z0-9_]+):\s*(.+)$`)
	for _, line := range strings.Split(string(b), "\n") {
		if m := reModel.FindStringSubmatch(line); m != nil {
			cur = m[1]
			models[cur] = map[string]any{}
			inDefaults = false
			continue
		}
		if cur == "" {
			continue
		}
		if strings.TrimSpace(line) == "defaults:" || regexp.MustCompile(`^    defaults:\s*$`).MatchString(line) {
			models[cur]["defaults"] = map[string]any{}
			inDefaults = true
			continue
		}
		if m := reField.FindStringSubmatch(line); m != nil && !inDefaults {
			models[cur][m[1]] = strings.Trim(m[2], `"' `)
			continue
		}
		if m := reDef.FindStringSubmatch(line); m != nil && inDefaults {
			v := strings.Trim(m[2], `"' `)
			var val any = v
			if i, err := strconv.Atoi(v); err == nil {
				val = i
			}
			defs := models[cur]["defaults"].(map[string]any)
			defs[m[1]] = val
		}
	}
	return models
}

func expandHome(p string) string {
	if strings.HasPrefix(p, "~/") {
		home, _ := os.UserHomeDir()
		return filepath.Join(home, p[2:])
	}
	return p
}

func asString(v any, def string) string {
	if v == nil {
		return def
	}
	switch t := v.(type) {
	case string:
		return t
	case int:
		return strconv.Itoa(t)
	case float64:
		return strconv.FormatFloat(t, 'f', -1, 64)
	default:
		return def
	}
}

func asInt(v any, def int) int {
	switch t := v.(type) {
	case int:
		return t
	case float64:
		return int(t)
	case string:
		if i, err := strconv.Atoi(t); err == nil {
			return i
		}
	}
	return def
}
