import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const testing = require("./testing.cjs");

export const {
  roundtripValue,
  roundtripInteger,
  roundtripBytes,
  roundtripString,
  measureConversion,
} = testing;

export default testing;
