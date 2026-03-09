<script lang="ts">
  let { navigate }: { navigate: (to: string) => void } = $props();

  let file = $state<File | null>(null);
  let uploading = $state(false);
  let error = $state('');

  function handleFile(e: Event) {
    file = (e.target as HTMLInputElement).files?.[0] ?? null;
    error = '';
  }

  async function upload() {
    if (!file) return;
    if (file.size > 1024 * 1024 * 1024) {
      error = 'File exceeds the 1GB limit';
      return;
    }
    uploading = true;
    error = '';

    const formData = new FormData();
    formData.append('video', file);

    try {
      const res = await fetch('/api/videos', { method: 'POST', body: formData });
      const body = await res.text();
      if (!res.ok) throw new Error(body);
      navigate(`/video/${body}`);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Upload failed';
      uploading = false;
    }
  }
</script>

<div class="page">
  <div class="card">
    <h1>Share a video</h1>
    <p class="subtitle">Upload a video and get a shareable link instantly.</p>

    <label class="file-label" class:has-file={!!file}>
      <input
        type="file"
        accept="video/mp4,video/quicktime,video/webm,video/x-matroska,video/x-msvideo"
        onchange={handleFile}
        disabled={uploading}
      />
      {#if file}
        <span>📎 {file.name}</span>
      {:else}
        <span>Choose a video file</span>
      {/if}
    </label>

    <button class="primary" onclick={upload} disabled={!file || uploading}>
      {uploading ? 'Uploading...' : 'Upload'}
    </button>

    {#if error}
      <p class="error">{error}</p>
    {/if}
  </div>
</div>

<style>
  .page {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    background: var(--bg-subtle);
  }

  .card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 2.5rem;
    width: 100%;
    max-width: 480px;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    text-align: center;
  }

  h1 {
    font-size: 1.75rem;
    font-weight: 700;
    color: var(--text);
  }

  .subtitle {
    color: var(--text-muted);
    font-size: 1rem;
  }

  .file-label {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    border: 2px dashed var(--border);
    border-radius: 12px;
    padding: 2rem;
    cursor: pointer;
    color: var(--text-muted);
    font-size: 1rem;
    transition: border-color 0.15s, color 0.15s;
  }

  .file-label:hover,
  .file-label.has-file {
    border-color: var(--blue);
    color: var(--blue);
  }

  .file-label input {
    display: none;
  }

  .error {
    color: #f87171;
    font-size: 0.9rem;
  }
</style>
