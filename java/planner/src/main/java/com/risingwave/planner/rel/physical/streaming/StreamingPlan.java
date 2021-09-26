package com.risingwave.planner.rel.physical.streaming;

/**
 * Plan for the stream execution. To be compatible with Calcite, a streaming plan is still a tree.
 * The root represents the result (sink, MV), and the leaf nodes represent the sources.
 *
 * <p>We remove the `serialize()` interface for StreamingPlan, as the serialization phase requires a
 * global id assigner. We defer the serialization implementation to later phases in the planning
 * procedure.
 */
public class StreamingPlan {
  private final RisingWaveStreamingRel streamingPlan;

  public StreamingPlan(RisingWaveStreamingRel streamingPlan) {
    this.streamingPlan = streamingPlan;
  }

  public RisingWaveStreamingRel getStreamingPlan() {
    return streamingPlan;
  }
}
