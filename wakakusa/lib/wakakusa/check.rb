# What an app writes that Wakakusa cannot take, named with the line and
# what to write instead.
#
# Every rule here stands for something the compiled run gets wrong: a
# shape that either fails to build or, worse, builds and then quietly
# behaves differently from the interpreted one. The gate catches those
# too, but only after a build; a refusal catches them while the app is
# still being written, and says what to write.
#
# Each rule names the reduction it came from, so when the compiler
# grows past one, the rule and its reason go together.

require "prism"
require "wakakusa/table"

module Wakakusa
  Refusal = Struct.new(:file, :line, :col, :message)

  class Check < Prism::Visitor
    def self.run(path)
      source = File.read(path)
      check = new(path, source)
      Prism.parse(source).value.accept(check)
      check.refusals
    end

    attr_reader :refusals

    def initialize(path, source)
      @path = path
      @lines = source.lines
      @refusals = []
      @block_params = []
      @in_view = 0
      @in_block = 0
      # The blocks open around here: an element's own, or a loop's.
      @block_kinds = []
      super()
    end

    # --- the rules ----------------------------------------------------------

    # A block on a container writes that container's children, so the
    # view is still being written inside it. A block anywhere else is a
    # handler, and a handler is where a write belongs.
    def visit_block_node(node)
      enter_block(node, false)
    end

    def enter_block(node, container, element: false)
      # A block written on an element, inside a loop's block: the
      # compiled run loses the loop's variables before it ever runs.
      if element && @block_kinds.include?(:loop)
        refuse(node, "a block on an element cannot be written inside a loop: a compiled run " \
                     "has lost the loop's variables by the time it runs. Move this into a " \
                     "method that takes what it needs (`def line(item, i)`), and call that " \
                     "from the loop")
      end
      @block_kinds.push(element ? :element : :loop)
      @block_params.push(block_parameter_names(node))
      @in_block += 1 unless container
      node.body&.accept(self)
      @in_block -= 1 unless container
      @block_params.pop
      @block_kinds.pop
    end

    # A global written from a block's own argument. The compiled run
    # keeps the type its first value gave the global and has nothing to
    # convert the block's argument to it, so the C does not build.
    def visit_global_variable_write_node(node)
      value = node.value
      if value.is_a?(Prism::LocalVariableReadNode) && @block_params.flatten.include?(value.name.to_s)
        refuse(node, "`#{node.name}` is a global, and a compiled run cannot write one from " \
                     "inside a block. The app's state belongs on the app: write `@#{node.name.to_s.delete_prefix("$")}` " \
                     "in a class with a `view` method, and hand it to `run`")
      end
      super
    end

    # A handler given as anything but the block or a symbol. A proc
    # handed through a keyword argument reaches the compiled run as its
    # address, and calls a handler with a number where a string was
    # meant.
    def visit_call_node(node)
      element = node.name.to_s
      keywords = keyword_arguments(node)
      if WK::ELEMENTS.key?(element) && node.receiver.nil?
        keywords.each do |assoc|
          name = key_name(assoc)
          next if name.nil?

          if WK::HANDLERS.include?(name)
            unless assoc.value.is_a?(Prism::SymbolNode)
              refuse(assoc, "`#{name}:` takes a symbol naming one of the app's own methods " \
                            "(`#{name}: :bump`), or leave it out and write the block instead. " \
                            "A compiled run receives a proc given here as a number")
            end
          elsif !WK::ELEMENTS.fetch(element).include?(name)
            refuse(assoc, "`#{element}` has no `#{name}:`. It takes " \
                          "#{WK::ELEMENTS.fetch(element).join(", ")}")
          end
        end
      end

      if node.block.is_a?(Prism::BlockNode)
        node.receiver&.accept(self)
        node.arguments&.accept(self)
        on_element = node.receiver.nil? && WK::ELEMENTS.key?(element)
        enter_block(node.block, on_element && WK::CONTAINERS.include?(element), element: on_element)
        return
      end
      super
    end

    # `@list = @list + [item]`. The two runs do not agree about what
    # that answers once the list is a field.
    def visit_instance_variable_write_node(node)
      if @in_view.positive? && @in_block.zero?
        refuse(node, "a view only reads. Move the write into a handler — the block on a " \
                     "button, or a method the app calls from one")
      end
      grown = grows_a_list?(node.value, node.name.to_s)
      if grown
        base = node.name.to_s.delete_prefix("@")
        refuse(node, "growing a list with `+ [...]` answers something different in a compiled " \
                     "run. Copy it and push: `#{base} = #{node.name}.dup`, `#{base}.push(...)`, " \
                     "`#{node.name} = #{base}`")
      end
      super
    end

    def visit_instance_variable_operator_write_node(node)
      if node.binary_operator == :+ && node.value.is_a?(Prism::ArrayNode)
        base = node.name.to_s.delete_prefix("@")
        refuse(node, "growing a list with `+= [...]` answers something different in a compiled " \
                     "run. Copy it and push: `#{base} = #{node.name}.dup`, `#{base}.push(...)`, " \
                     "`#{node.name} = #{base}`")
      end
      super
    end

    def visit_def_node(node)
      @in_view += 1 if node.name == :view
      super
      @in_view -= 1 if node.name == :view
    end

    # --- the parts ----------------------------------------------------------

    private

    def block_parameter_names(node)
      params = node.parameters
      return [] unless params.respond_to?(:parameters) && params.parameters

      params.parameters.requireds.filter_map do |p|
        p.name.to_s if p.respond_to?(:name)
      end
    end

    def keyword_arguments(node)
      args = node.arguments&.arguments || []
      args.grep(Prism::KeywordHashNode).flat_map(&:elements).grep(Prism::AssocNode)
    end

    def key_name(assoc)
      key = assoc.key
      return key.unescaped if key.is_a?(Prism::SymbolNode)

      nil
    end

    def grows_a_list?(value, name)
      value.is_a?(Prism::CallNode) &&
        value.name == :+ &&
        value.receiver.is_a?(Prism::InstanceVariableReadNode) &&
        value.receiver.name.to_s == name &&
        value.arguments&.arguments&.first.is_a?(Prism::ArrayNode)
    end

    def refuse(node, message)
      loc = node.location
      @refusals << Refusal.new(@path, loc.start_line, loc.start_column, message)
    end
  end

  # `file:line:col: …`, the line it is on, and a caret under it.
  def self.render(refusal, lines)
    head = "#{refusal.file}:#{refusal.line}:#{refusal.col + 1}: " \
           "Wakakusa cannot take this — #{refusal.message}"
    src = lines[refusal.line - 1]
    return head if src.nil?

    "#{head}\n    #{src.chomp}\n    #{" " * refusal.col}^"
  end
end
