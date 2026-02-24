// handle_registry.h
//
// OCCT-specific handle registry interface.
//
// This header requires OCCT includes to be on the include path.  It wraps
// HandleRegistryBase<TopoDS_Shape, Handle(Poly_Triangulation)> with a set of
// free functions that form the internal C++ API consumed by cam_geometry.cpp.
//
// Usage (C++ only — not part of the plain-C public API):
//
//   #include "handle_registry.h"
//
//   TopoDS_Shape shape = ...;
//   uint64_t id = registry_store_shape(shape);
//   const TopoDS_Shape& s = registry_get_shape(id);
//   registry_free_shape(id);

#pragma once

#include <cstdint>

// OCCT includes — must be available on the include path.
#include <Poly_Triangulation.hxx>
#include <Standard_Handle.hxx>
#include <TopoDS_Shape.hxx>

#include "handle_registry_base.h"

// Convenience alias for the OCCT mesh handle type.
using OcctMeshHandle = opencascade::handle<Poly_Triangulation>;

// Type alias for the global registry instantiation.
using OcctHandleRegistry = HandleRegistryBase<TopoDS_Shape, OcctMeshHandle>;

// ---------------------------------------------------------------------------
// Shape registry operations
// ---------------------------------------------------------------------------

// Store a copy of shape and return its opaque ID.
uint64_t registry_store_shape(const TopoDS_Shape& shape);

// Retrieve the stored shape by ID.
// Throws std::out_of_range for an invalid or already-freed ID.
const TopoDS_Shape& registry_get_shape(uint64_t id);

// Remove shape from registry.  Safe to call on an already-freed ID.
void registry_free_shape(uint64_t id);

// ---------------------------------------------------------------------------
// Mesh registry operations
// ---------------------------------------------------------------------------

// Store a mesh handle and return its opaque ID.
uint64_t registry_store_mesh(OcctMeshHandle mesh);

// Retrieve the stored mesh handle by ID.
// Throws std::out_of_range for an invalid or already-freed ID.
const OcctMeshHandle& registry_get_mesh(uint64_t id);

// Remove mesh from registry.  Safe to call on an already-freed ID.
void registry_free_mesh(uint64_t id);
