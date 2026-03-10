<script lang="ts">
  import Hls from 'hls.js';
  import { onMount } from 'svelte';

  let { token, navigate }: { token: string; navigate: (to: string) => void } = $props();

  let status = $state('processing');
  let notFound = $state(false);
  let videoEl = $state<HTMLVideoElement>(null!);
  let toastVisible = $state(false);

  let shareUrl = $derived(`${window.location.origin}/video/${token}`);
  let hls: Hls | null = null;

  function initPlayer() {
    const src = `/api/videos/${token}/playlist.m3u8`;
    if (Hls.isSupported()) {
      hls = new Hls({
        manifestLoadingTimeOut: 10000,
        manifestLoadingMaxRetry: 20,
        manifestLoadingRetryDelay: 1000,
        lowLatencyMode: true,
        maxBufferLength: 3,
      });
      hls.loadSource(src);
      hls.attachMedia(videoEl);
      hls.on(Hls.Events.MANIFEST_PARSED, () => videoEl.play());
    } else if (videoEl.canPlayType('application/vnd.apple.mpegurl')) {
      videoEl.src = src;
    }
  }

  async function poll(): Promise<boolean> {
    const res = await fetch(`/api/videos/${token}`);
    if (res.status === 404) { notFound = true; return false; }
    if (!res.ok) return false;
    const data = await res.json();
    status = data.status;
    return true;
  }

  async function copyLink() {
    try {
      await navigator.clipboard.writeText(shareUrl);
    } catch {
      // Fallback for non-secure contexts (plain HTTP)
      const el = document.createElement('textarea');
      el.value = shareUrl;
      el.style.position = 'fixed';
      el.style.opacity = '0';
      document.body.appendChild(el);
      el.select();
      document.execCommand('copy');
      document.body.removeChild(el);
    }
    toastVisible = true;
    setTimeout(() => (toastVisible = false), 2500);
  }

  onMount(() => {
    const interval = setInterval(async () => {
      const ok = await poll();
      if (!ok || status === 'ready') {
        clearInterval(interval);
        if (status === 'ready') initPlayer();
      }
    }, 3000);

    poll().then((ok) => {
      if (!ok || status === 'ready') {
        clearInterval(interval);
        if (status === 'ready') initPlayer();
      }
    });

    return () => {
      clearInterval(interval);
      hls?.destroy();
    };
  });
</script>

<div class="page">
  {#if notFound}
    <div class="processing-card">
      <p>Video not found.</p>
      <button class="ghost" onclick={() => navigate('/')}>Go home</button>
    </div>
  {:else if status === 'uploading'}
    <div class="processing-card">
      <div class="spinner"></div>
      <p>Uploading your video...</p>
      <span class="status-badge">uploading</span>
    </div>
  {:else if status === 'processing'}
    <div class="processing-card">
      <div class="spinner"></div>
      <p>Transcoding your video...</p>
      <span class="status-badge">transcoding</span>
    </div>
  {:else if status === 'failed'}
    <div class="processing-card">
      <p>Something went wrong.</p>
      <button class="ghost" onclick={() => navigate('/')}>Try again</button>
    </div>
  {:else}
    <div class="player-wrapper">
      <video bind:this={videoEl} controls autoplay></video>
    </div>
  {/if}

  {#if !notFound}
    <div class="share-bar">
      <span class="share-url">{shareUrl}</span>
      <button class="primary" onclick={copyLink}>Copy link</button>
      <button class="ghost" onclick={() => navigate('/')}>Upload another</button>
    </div>
  {/if}
</div>

{#if toastVisible}
  <div class="toast">Link copied!</div>
{/if}

<style>
  .page {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1.5rem;
    padding: 2rem;
    background: var(--bg-subtle);
  }

  .player-wrapper {
    width: 100%;
    max-width: 1100px;
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 8px 40px rgba(0, 0, 0, 0.4);
  }

  video {
    width: 100%;
    display: block;
  }

  .processing-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 3rem 5rem;
    color: var(--text-muted);
    font-size: 1.05rem;
  }

  .spinner {
    width: 40px;
    height: 40px;
    border: 3px solid var(--blue-light);
    border-top-color: var(--blue);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .status-badge {
    font-size: 0.78rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    background: var(--blue);
    color: #fff;
    padding: 0.25rem 0.8rem;
    border-radius: 9999px;
  }

  .share-bar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 9999px;
    padding: 0.4rem 0.4rem 0.4rem 1.5rem;
    width: 100%;
    max-width: 1100px;
  }

  .share-url {
    flex: 1;
    font-family: monospace;
    font-size: 1rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toast {
    position: fixed;
    bottom: 2rem;
    left: 50%;
    transform: translateX(-50%);
    background: var(--bg-card);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 0.65rem 1.5rem;
    border-radius: 9999px;
    font-size: 0.95rem;
    font-weight: 500;
    pointer-events: none;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  }
</style>
