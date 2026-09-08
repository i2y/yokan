// Objects that point at one another. Yokan needs a weak back pointer
// here so a parent and a child cannot own each other forever; Go's
// collector takes a cycle in its stride, so the back pointer is an
// ordinary pointer and the tree is written the way it reads.
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Node struct {
	label  string
	kid    *Node
	parent *Node
}

type Tree struct {
	root *Node
	keep *Node
	note string
}

func (t *Tree) build() {
	a := &Node{label: "alpha"}
	b := &Node{label: "beta"}
	a.kid = b
	b.parent = a
	t.root = a
	t.keep = b
}

func (t *Tree) peek() {
	switch {
	case t.root == nil && t.keep == nil:
		t.note = "no root"
	case t.root == nil:
		t.note = fmt.Sprintf("kept %s, parent=%s", t.keep.label, t.keep.parent.label)
	case t.root.kid == nil:
		t.note = "no kid"
	default:
		t.note = fmt.Sprintf("kid=%s parent=%s", t.root.kid.label, t.root.kid.parent.label)
	}
}

func (t *Tree) View() Element {
	rootLine := Text("root: (none)")
	if t.root != nil {
		rootLine = Text("root: " + t.root.label)
	}
	return Column(
		Text("note: "+t.note),
		rootLine,
		Row(
			Button("build").OnClick(func() { t.build() }),
			Button("peek").OnClick(func() { t.peek() }),
			Button("drop").OnClick(func() { t.root = nil }),
		).Spacing(6),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Tree{note: "-"}, Title("links"))
}
