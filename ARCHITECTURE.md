# NexusOS Architecture: The Deep Dive

## Kernel Philosophy

NexusOS adopts a radical "LibOS" philosophy. Traditional kernels act as arbiters, managing resources indiscriminately for all processes. In NexusOS, the kernel is not a separate entity that you make requests to; it is a library that you link against.

### Link-Time Optimization (LTO)
When you build an application for NexusOS ("The Unikernel"), the compiler pulls in *only* the specific kernel functions your code uses.
- If your app doesn't use the network stack, the network stack is never compiled into the binary.
- This results in extremely small, cache-friendly binaries.
- **Zero Context Switches**: Since there are no rings to cross (everything runs in Ring-0) and no other processes to schedule, the concept of a context switch is eliminated.

## The "Cortex" Engine

Cortex is the intelligence layer embedded directly above the hypervisor. It is designed to run Large Language Models (LLMs) effectively on standard CPUs.

### Context Paging
One of the biggest bottlenecks in LLM inference is managing the KV (Key-Value) cache for the context window.
- **Traditional Approach**: Keeps the entire context in VRAM or RAM, limiting the maximum sequence length.
- **NexusOS Approach**: We implement "Context Paging." We reserve the highest-speed system RAM for the *active* layers of the model's context window. As the model progresses, inactive blocks of the context are swapped out to distinct namespaces on high-speed NVMe SSDs via direct DMA (Direct Memory Access). This essentially allows for "infinite" context windows, bounded only by disk space, with minimal performance penalty.

### BitNet b1.58 Integration
Standard LLMs use 16-bit or 32-bit floating-point numbers, requiring massive matrix multiplications.
- **Optimization**: Cortex uses **BitNet b1.58**, a 1-bit quantization technique.
- **Result**: This reduces the heavy matrix multiplication operations (MatMul) into simple efficient additions.
- **CPU Efficiency**: This allows purely CPU-based inference (using AVX-512 vector instructions) to compete with GPU performance for many tasks, effectively democratizing local AI.

## MemexFS Internals

MemexFS is a vector-based semantic file system that fundamentally changes how data is stored and retrieved.

### From Inodes to Vectors
- **Legacy FS**: Uses Inodes to point to blocks of data on disk, organized in a tree hierarchy.
- **MemexFS**: Uses **1536-dimensional Vector Embeddings**. When you save a file, Cortex analyzes its content and generates a vector representation. This vector is stored in an embedded instance of the **Qdrant** vector database.
- **Retrieval**: Files are retrieved by similarity search (`Nearest Neighbor`).

### The "Virtual VFS" Layer
We understand that legacy tools (like compilers, editors, and runtimes) still expect a file tree.
- **Solution**: MemexFS implements a **"Virtual VFS" layer**.
- **Mechanism**: When a legacy application asks to list a directory, MemexFS dynamically generates a virtual folder structure based on the query or metadata.
- **Example**: Opening `/mnt/memex/projects` might trigger a query for all objects tagged "project," presenting them as folders to the application, while physically they are scattered on disk, indexed only by their vector IDs.
