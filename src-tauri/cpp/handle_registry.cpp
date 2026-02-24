// handle_registry.cpp
//
// OCCT-specific handle registry implementation.
//
// A single global OcctHandleRegistry instance (Meyers singleton) is shared
// by all cam_geometry.cpp functions.  It is created on first use and destroyed
// at program exit in the correct static-destruction order.

#include "handle_registry.h"

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------

static OcctHandleRegistry& global_registry() {
    static OcctHandleRegistry registry;
    return registry;
}

// ---------------------------------------------------------------------------
// Shape operations
// ---------------------------------------------------------------------------

uint64_t registry_store_shape(const TopoDS_Shape& shape) {
    return global_registry().store_shape(shape);
}

const TopoDS_Shape& registry_get_shape(uint64_t id) {
    return global_registry().get_shape(id);
}

void registry_free_shape(uint64_t id) {
    global_registry().free_shape(id);
}

// ---------------------------------------------------------------------------
// Mesh operations
// ---------------------------------------------------------------------------

uint64_t registry_store_mesh(OcctMeshHandle mesh) {
    return global_registry().store_mesh(mesh);
}

const OcctMeshHandle& registry_get_mesh(uint64_t id) {
    return global_registry().get_mesh(id);
}

void registry_free_mesh(uint64_t id) {
    global_registry().free_mesh(id);
}
