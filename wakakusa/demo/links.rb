# Objects that point at one another. Yokan needs a weak back pointer
# here so a parent and a child cannot own each other forever; Ruby's
# collector takes a cycle in its stride, so the back pointer is an
# ordinary reference and the tree is written the way it reads.
require "wakakusa"

class Node
  attr_accessor :label, :kid, :parent

  def initialize(label)
    @label = label
    @kid = nil
    @parent = nil
  end
end

class Tree
  def initialize
    @root = nil
    @keep = nil
    @note = "-"
  end

  def build
    a = Node.new("alpha")
    b = Node.new("beta")
    a.kid = b
    b.parent = a
    @root = a
    @keep = b
  end

  def peek
    @note = if @root.nil?
              @keep.nil? ? "no root" : "kept #{@keep.label}, parent=#{@keep.parent.label}"
            elsif @root.kid.nil?
              "no kid"
            else
              "kid=#{@root.kid.label} parent=#{@root.kid.parent.label}"
            end
  end

  def view
    column(spacing: 8.0, padding: 12.0) {
      text "note: #{@note}"
      if @root.nil?
        text "root: (none)"
      else
        text "root: #{@root.label}"
      end
      row(spacing: 6.0) {
        button("build") { build }
        button("peek") { peek }
        button("drop") { @root = nil }
      }
    }
  end
end

run(Tree.new, title: "links")
