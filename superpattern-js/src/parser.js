/**
 * .epe File Parser
 *
 * This module provides functions to parse Pixelblaze .epe files
 * and extract the JavaScript source code.
 */

/**
 * Extract JavaScript code from .epe file format
 * @param {Object} epeContent - Parsed .epe file content
 * @returns {string} JavaScript source code
 */
function extractJavaScriptFromEpe(epeContent) {
  if (!epeContent.sources || !epeContent.sources.main) {
    throw new Error("Invalid .epe file format: missing sources.main");
  }
  return epeContent.sources.main;
}

/**
 * Parse .epe file from JSON string
 * @param {string} epeJson - JSON string content of .epe file
 * @returns {Object} Parsed .epe content
 */
function parseEpeFile(epeJson) {
  try {
    return JSON.parse(epeJson);
  } catch (error) {
    throw new Error(`Failed to parse .epe file: ${error.message}`);
  }
}

module.exports = {
  extractJavaScriptFromEpe,
  parseEpeFile,
};
