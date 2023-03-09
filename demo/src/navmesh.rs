pub struct NavMesh {}

impl NavMesh {
    pub fn new(doc: &gltf::Document, buffers: &[gltf::buffer::Data]) -> Self {
        let module1 = doc
            .nodes()
            .find(|node| Some("module1") == node.name())
            .unwrap();

        let get_buffer_data = |buffer: gltf::Buffer| -> Option<&[u8]> {
            buffers.get(buffer.index()).map(std::ops::Deref::deref)
        };

        let get_accessor_data = |accessor: gltf::Accessor| -> Option<&[u8]> {
            let view = accessor.view()?;

            let start = view.offset();
            let end = start + view.length();

            let buffer = get_buffer_data(view.buffer())?;

            Some(&buffer[start..end])
        };

        traverse_nodes_tree(
            std::iter::once(module1),
            &mut |_, node| {
                if let Some(mesh) = node.mesh() {
                    for primitive in mesh.primitives() {
                        let reader = primitive.reader(get_buffer_data);

                        let indices = reader
                            .read_indices()
                            .unwrap()
                            .into_u32()
                            .collect::<Vec<_>>();

                        let vertices = reader
                            .read_positions()
                            .unwrap()
                            .map(glam::Vec3::from_array)
                            .collect::<Vec<_>>();
                    }
                }

                dbg!(node);
            },
            (),
        );

        Self {}
    }
}

fn traverse_nodes_tree<'a, T>(
    nodes: impl Iterator<Item = gltf::Node<'a>>,
    cb: &mut dyn FnMut(&T, &gltf::Node) -> T,
    acc: T,
) {
    for node in nodes {
        let res = cb(&acc, &node);
        traverse_nodes_tree(node.children(), cb, res);
    }
}
