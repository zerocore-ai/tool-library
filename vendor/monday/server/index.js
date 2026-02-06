/**
 * monday.com MCPB entry point.
 *
 * This bundle vendors the built monday.com MCP server (CommonJS) under
 * `server/monday-api-mcp/dist/`. The upstream server parses config from CLI args
 * (process.argv) and environment variables (e.g. MONDAY_TOKEN).
 */

'use strict';

require('./monday-api-mcp/dist/index.js');
