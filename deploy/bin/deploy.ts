#!/usr/bin/env node
import 'source-map-support/register';
import * as cdk from '@aws-cdk/core';
import { WarpStack } from '../lib/warp';

const app = new cdk.App();

new WarpStack(app, 'warp-us-east-1', {
  env: { account: '864566598233', region: 'us-east-1' },
});

new WarpStack(app, 'warp-us-west-2', {
  env: { account: '864566598233', region: 'us-west-2' },
});

app.synth();