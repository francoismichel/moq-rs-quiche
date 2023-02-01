import * as cdk from '@aws-cdk/core';
import * as ec2 from "@aws-cdk/aws-ec2";            // Allows working with EC2 and VPC resources
import * as iam from "@aws-cdk/aws-iam";            // Allows working with IAM resources
import * as ecs from "@aws-cdk/aws-ecs";
import * as cwl from "@aws-cdk/aws-logs";
import * as elb from "@aws-cdk/aws-elasticloadbalancingv2"
import * as ecr_assets from '@aws-cdk/aws-ecr-assets';
import * as route53 from "@aws-cdk/aws-route53";
import * as route53_targets from "@aws-cdk/aws-route53-targets"
import * as path from "path";                       // Helper for working with file paths

export class WarpStack extends cdk.Stack {
  constructor(scope: cdk.Construct, id: string, props: cdk.StackProps) {
    super(scope, id, props);

    //1. Create VPC
    const vpc = new ec2.Vpc(this, 'VPC');

    //2. Creation of Execution Role for our task
    const execRole = new iam.Role(this, 'warp-exec-role', {
      roleName: 'warp-exec-role', assumedBy: new iam.ServicePrincipal('ecs-tasks.amazonaws.com')
    })

    //3. Adding permissions to the above created role...basically giving permissions to ECR image and Cloudwatch logs
    execRole.addToPolicy(new iam.PolicyStatement({
      actions: [
        "ecr:GetAuthorizationToken",
        "ecr:BatchCheckLayerAvailability",
        "ecr:GetDownloadUrlForLayer",
        "ecr:BatchGetImage",
        "logs:CreateLogStream",
        "logs:PutLogEvents"
      ], effect: iam.Effect.ALLOW, resources: ["*"]
    }));

    //4. Create the ECS fargate cluster
    const cluster = new ecs.Cluster(this, 'warp-cluster', { vpc, clusterName: "warp-cluster" });

    //5. Create a task definition for our cluster to invoke a task
    const taskDef = new ecs.FargateTaskDefinition(this, "warp-task", {
      family: 'warp-task',
      executionRole: execRole,
      taskRole: execRole,
      volumes: [{ name: "cert" }],
    });

    //6. Create log group for our task to put logs
    const log_group = cwl.LogGroup.fromLogGroupName(this, 'warp-log-group', '/ecs/warp-task');

    const log = new ecs.AwsLogDriver({
      logGroup: log_group ? log_group : new cwl.LogGroup(this, 'warp-log-group', { logGroupName: '/ecs/warp-task' }),
      streamPrefix: 'ecs'
    })

    // ?? Build the docker images
    const player_image = new ecr_assets.DockerImageAsset(this, 'warp-player-image', {
      directory: path.join(__dirname, '/../../player'),
    });

    const server_image = new ecr_assets.DockerImageAsset(this, 'warp-server-image', {
      directory: path.join(__dirname, '/../../server'),
    });

    /*
    const media_image = new ecr_assets.DockerImageAsset(this, 'warp-media-image', {
      directory: path.join(__dirname, '/../../media'),
    });
    */

    //7. Create container for the task definition from ECR image
    var player_container = taskDef.addContainer("warp-player-container", {
      image: ecs.ContainerImage.fromDockerImageAsset(player_image),
      logging: log,
      portMappings: [{
        containerPort: 80,
        hostPort: 80,
        protocol: ecs.Protocol.TCP
      }, {
        containerPort: 443,
        hostPort: 443,
        protocol: ecs.Protocol.TCP
      }],
    })

    var server_container = taskDef.addContainer("warp-server-container", {
      image: ecs.ContainerImage.fromDockerImageAsset(server_image),
      logging: log,
      portMappings: [{
        containerPort: 443,
        hostPort: 443,
        protocol: ecs.Protocol.UDP
      }],
    })

    //9. Create the NLB using the above VPC.
    const nlb = new elb.NetworkLoadBalancer(this, 'warp-nlb', {
      loadBalancerName: 'warp-nlb',
      vpc,
      internetFacing: true,
    });

    // ?? Create a DNS entry for the nlb 
    const zone = route53.HostedZone.fromHostedZoneAttributes(this, "warp-zone", {
      hostedZoneId: "Z0166044CZ3H7MIDXT7P", // Unfortunately we need to hard-code the route53 zone
      zoneName: "quic.video",
    });

    const record = new route53.ARecord(this, "warp-record", {
        zone: zone,
        target: route53.RecordTarget.fromAlias(new route53_targets.LoadBalancerTarget(nlb)),
    })

    // Route based on latency.
    const recordSet = (record.node.defaultChild as route53.CfnRecordSet);
    recordSet.region = props.env?.region;
    recordSet.setIdentifier = props.env?.region;

    //10. Add a listener on a particular port for the NLB
    const http_listener = nlb.addListener('warp-http-listener', {
      protocol: elb.Protocol.TCP,
      port: 80,
    });

    const https_listener = nlb.addListener('warp-https-listener', {
      protocol: elb.Protocol.TCP_UDP,
      port: 443,
    });

    //11. Create your own security Group using VPC
    const sg = new ec2.SecurityGroup(this, 'warp-player-sg', {
      securityGroupName: "warp-player-sg",
      vpc: vpc,
      allowAllOutbound: true,
    });

    //12. Add IngressRule to access the docker image
    sg.addIngressRule(ec2.Peer.ipv4('0.0.0.0/0'), ec2.Port.tcp(80), 'HTTP');
    sg.addIngressRule(ec2.Peer.ipv4('0.0.0.0/0'), ec2.Port.tcp(443), 'HTTPS');
    sg.addIngressRule(ec2.Peer.ipv4('0.0.0.0/0'), ec2.Port.udp(443), 'HTTP/3');

    //13. Create Fargate Service from cluster, task definition and the security group
    const service = new ecs.FargateService(this, 'warp-service', {
      cluster: cluster,
      taskDefinition: taskDef,
      assignPublicIp: true,
      serviceName: "warp-service",
      securityGroups: [sg],
    });

    //14. Add fargate service to the listener 
    http_listener.addTargets('warp-http-target', {
      targetGroupName: 'warp-http-target',
      protocol: elb.Protocol.TCP,
      port: 80,
      targets: [service],
      deregistrationDelay: cdk.Duration.seconds(300)
    });

    https_listener.addTargets('warp-https-target', {
      targetGroupName: 'warp-https-target',
      protocol: elb.Protocol.TCP_UDP,
      port: 443,
      targets: [service],
      deregistrationDelay: cdk.Duration.seconds(300)
    });
  }
}
