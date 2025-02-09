Project Goals:
- To expose a graphic user interface for exploring and manipulating schemaful graphs.
  - Specifically in this case, the schemaful graphs will consist of `nodes` (or `operatives`) which represent subgraph templates. Each node may have n number of `slot`s, which represent a typed collection of edges. Each slot may be constrained by cardinality (e.g. exactly 1 edge, between 0 and 3 edges, etc.) and by allowed node type(s).
  - Visually, a node may have ports which correspond to its slots, from which the corresponding edges will flow.
- Make the developer API surface simple enough to allow any wasm framework (and maybe JS framework) to easily wrap the functionality in an idiomatic component of the library.
  - Setup function which, in a minimal setup, only requires the user to pass in a node ref which will be inhabited by the canvas
  - Setup function to accept schema of possible graph elements which can be added (ideally making use of the existing [molecule_schema](https://github.com/reedwoodruff/molecule_schema) format)

Design questions:
- Given a constraint schema, should there be a way to pick and choose which elements of the schema to expose in the GUI? Or should it be assumed that all of the operatives will be made available?
- How best to represent the intermediate steps which the construction of a graph requires? For example, given a node with a slot which requires 2 edges, the user needs to make them sequentially, which leaves the graph in an intermediate, invalid, state.
  - This would be less of an issue if not relying on the mechanisms exposed in `molecule_schema` for dealing with a managed graph. Ideally, though, this kind of robust construction process would be supported and faciliated within that system and then utilized in this one (rather than making a one-off implementation here).
  - It seems like the end goal would be to either ultimately return a finished, schema-compliant graph at the end of the GUI editing process, or to *sync* to some existing one (either periodically throughout the editing process if the requisite "in-construction" features are built, or once at the completion if they are not)

Not too worried at the moment about customizability of styles/appearance, though this could be nice at some point.

Rough end-goal sketch/inspiration: React Flow
