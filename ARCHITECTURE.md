# Hot Potato

## Overview

An anonymous video sharing platform built with Rust and Svelte.

## Components

### Frontend - Svelte SPA

functional UI for uploading and streaming videos

### Backend - Rust/Axum server

Offers endpoints for uploading videos and consuming videos
manages these resources:

- videos
- share-tokens

### Storage - PostgreSQL

Stores the mapping between share-token and HLS segments in BLOB storage

### BLOB Storage - MinIO / S3

Stores The video's streamable parts in HLS segments

## Endpoints

POST /api/video
GET /api/video/{share-token}

## Data flow

Video upload:
browser -> frontend -> backend -> blob storage

Video streaming:
browser -> frontend -> backend -> blob storage

## Technology Decisions

Docker:

MinIO:

HLS:

Axum:

Svelte:

## Scaling
