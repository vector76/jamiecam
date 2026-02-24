// handle_registry_base.h
//
// Template implementation of the handle registry.  This header has no
// dependency on OCCT so it can be compiled and tested independently.
//
// The public OCCT-specific interface is in handle_registry.h, which wraps
// this template with TopoDS_Shape and Handle(Poly_Triangulation).
//
// Design:
//   - IDs start at 1.  0 is always the null handle (CG_NULL_ID).
//   - A single atomic counter generates IDs for both shapes and meshes,
//     guaranteeing that every handle in the system is globally unique.
//   - std::shared_mutex provides concurrent read / exclusive write access.
//   - ShapeT must be copy-constructible (TopoDS_Shape satisfies this via
//     OCCT's internal ref-counting).
//   - MeshT must be copy-constructible (opencascade::handle<T> is a
//     ref-counted smart pointer and satisfies this).

#pragma once

#include <atomic>
#include <cstdint>
#include <mutex>
#include <shared_mutex>
#include <stdexcept>
#include <unordered_map>

template <typename ShapeT, typename MeshT>
class HandleRegistryBase {
public:
    HandleRegistryBase() : next_id_(1) {}

    // Non-copyable, non-movable (singleton semantics).
    HandleRegistryBase(const HandleRegistryBase&)            = delete;
    HandleRegistryBase& operator=(const HandleRegistryBase&) = delete;

    // ---------------------------------------------------------------------------
    // Store
    // ---------------------------------------------------------------------------

    // Copy shape into the registry and return its ID.  Thread-safe.
    uint64_t store_shape(const ShapeT& shape) {
        uint64_t id = next_id_.fetch_add(1, std::memory_order_relaxed);
        std::unique_lock<std::shared_mutex> lock(mutex_);
        shapes_.emplace(id, shape);
        return id;
    }

    // Copy mesh into the registry and return its ID.  Thread-safe.
    uint64_t store_mesh(const MeshT& mesh) {
        uint64_t id = next_id_.fetch_add(1, std::memory_order_relaxed);
        std::unique_lock<std::shared_mutex> lock(mutex_);
        meshes_.emplace(id, mesh);
        return id;
    }

    // ---------------------------------------------------------------------------
    // Retrieve
    // ---------------------------------------------------------------------------

    // Return a const reference to the stored shape.
    // Throws std::out_of_range if id is not present.
    //
    // Safety invariant (enforced by the Rust wrapper layer): the caller must
    // not hold this reference across a concurrent free_shape() call on the
    // same id.  Rust ownership guarantees this in practice.
    const ShapeT& get_shape(uint64_t id) const {
        std::shared_lock<std::shared_mutex> lock(mutex_);
        auto it = shapes_.find(id);
        if (it == shapes_.end())
            throw std::out_of_range("HandleRegistry: invalid shape ID");
        return it->second;
    }

    // Return a const reference to the stored mesh handle.
    // Throws std::out_of_range if id is not present.
    const MeshT& get_mesh(uint64_t id) const {
        std::shared_lock<std::shared_mutex> lock(mutex_);
        auto it = meshes_.find(id);
        if (it == meshes_.end())
            throw std::out_of_range("HandleRegistry: invalid mesh ID");
        return it->second;
    }

    // ---------------------------------------------------------------------------
    // Free
    // ---------------------------------------------------------------------------

    // Remove shape from registry.  Returns true if removed, false if not found.
    bool free_shape(uint64_t id) {
        std::unique_lock<std::shared_mutex> lock(mutex_);
        return shapes_.erase(id) > 0;
    }

    // Remove mesh from registry.  Returns true if removed, false if not found.
    bool free_mesh(uint64_t id) {
        std::unique_lock<std::shared_mutex> lock(mutex_);
        return meshes_.erase(id) > 0;
    }

    // ---------------------------------------------------------------------------
    // Introspection (primarily for tests)
    // ---------------------------------------------------------------------------

    size_t shape_count() const {
        std::shared_lock<std::shared_mutex> lock(mutex_);
        return shapes_.size();
    }

    size_t mesh_count() const {
        std::shared_lock<std::shared_mutex> lock(mutex_);
        return meshes_.size();
    }

private:
    mutable std::shared_mutex                 mutex_;
    std::atomic<uint64_t>                     next_id_;
    std::unordered_map<uint64_t, ShapeT>      shapes_;
    std::unordered_map<uint64_t, MeshT>       meshes_;
};
