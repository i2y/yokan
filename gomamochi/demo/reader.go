// A page fetched and read, with nothing outside the machine involved:
// the app serves the document to itself on a port the operating system
// picks, so both runs read the same bytes and the gate can compare
// them. The fetch happens off the window's thread, which is what
// `Task` is for; the server answers on a goroutine of its own.
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"

	. "github.com/i2y/yokan/gomamochi"
)

const body = `{"items": [` +
	`{"title": "gomamochi ships native go apps", "points": 128},` +
	`{"title": "one engine, two runs", "points": 64},` +
	`{"title": "the gate arbitrates", "points": 256}` +
	`]}`

type Reader struct {
	status string
	titles []string
	top    string
	port   int
}

// A server that answers exactly one request and then closes.
func (r *Reader) serveOne() {
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		r.status = "no port"
		return
	}
	r.port = ln.Addr().(*net.TCPAddr).Port
	go func() {
		conn, err := ln.Accept()
		if err != nil {
			return
		}
		// past the request head: a blank line is where it ends
		in := bufio.NewReader(conn)
		for {
			line, err := in.ReadString('\n')
			if err != nil || line == "\r\n" {
				break
			}
		}
		fmt.Fprintf(conn, "HTTP/1.1 200 OK\r\nContent-Length: %d\r\nConnection: close\r\n\r\n%s", len(body), body)
		conn.Close()
		ln.Close()
	}()
}

func (r *Reader) fetch() {
	r.status = "fetching"
	r.serveOne()
	url := fmt.Sprintf("http://127.0.0.1:%d/", r.port)
	Task(func() any {
		resp, err := http.Get(url)
		if err != nil {
			return ""
		}
		defer resp.Body.Close()
		text, _ := io.ReadAll(resp.Body)
		return string(text)
	}, func(v any) { r.read(v.(string)) })
}

func (r *Reader) read(text string) {
	var doc map[string]any
	if err := json.Unmarshal([]byte(text), &doc); err != nil {
		r.status = "unreadable"
		return
	}
	items, _ := doc["items"].([]any)
	r.titles = nil
	bestTitle, bestPoints := "", -1.0
	for _, it := range items {
		item, _ := it.(map[string]any)
		title, _ := item["title"].(string)
		points, _ := item["points"].(float64)
		r.titles = append(r.titles, title)
		if points > bestPoints {
			bestTitle, bestPoints = title, points
		}
	}
	r.top = fmt.Sprintf("%s (%d)", bestTitle, int(bestPoints))
	r.status = fmt.Sprintf("read %d items", len(items))
}

func (r *Reader) line(i int) Element {
	return Text(r.titles[i]).Size(13)
}

func (r *Reader) View() Element {
	return Column(
		Text("reader").Size(18).Bold(true),
		Text("status: "+r.status).Size(12).Color("#8a8f98"),
		Text("top: "+r.top),
		ListView(len(r.titles), func(i int) Element { return r.line(i) }).ItemHeight(22).Height(80),
		Button("fetch").OnClick(func() { r.fetch() }),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Reader{status: "idle", top: "-"}, Title("reader"))
}
